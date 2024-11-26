use anyhow::{anyhow, bail, Context as _, Result};
use console::{pad_str, style, Alignment, StyledObject};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::path::Path;

use crate::args::{Args, Command};
use crate::db::{
    Account, AccountId, AccountType, AddOrVerifyResult, Amount, BeancountAccountInfo,
    PlaidAccountInfo, Transaction,
};
use crate::export::export_transactions;
use crate::terminal::{self, BulletPointPrinter, LineWriter};

use super::db::{self, BankConnection, Cipher, DatabaseV1, DbPlaidAuth, XChaCha20Poly1305Cipher};
use super::plaid_api;

// TODO Configurable DB Location
const DB_PATH: &str = "beancount_plaid.db";

// TODO Configurable encryption key
fn db_key() -> chacha20poly1305::Key {
    let mut rng = StdRng::seed_from_u64(1);
    let mut key_bytes = [0; 32];
    rng.fill_bytes(&mut key_bytes);
    key_bytes.into()
}

pub async fn main(args: Args) -> Result<()> {
    let mut cli = match args.command {
        Command::Init => Cli::new_init_db().await?,
        _ => Cli::new_load_db().await?,
    };
    match args.command {
        Command::Init => cli.main_init().await?,
        Command::AddConnection => cli.main_add_connection().await?,
        Command::ListConnections => cli.main_list_connections().await?,
        Command::Sync => cli.main_sync().await?,
        Command::ListTransactions => cli.main_list_transactions().await?,
        Command::ExportAll => cli.main_export_all_transactions().await?,
        Command::ExportNew => cli.main_export_new_transactions().await?,
    }
    cli.save_db().await?;
    Ok(())
}

pub struct Cli {
    db: DatabaseV1,
    db_cipher: XChaCha20Poly1305Cipher,
    plaid_api: plaid_api::Plaid,
}

impl Cli {
    pub async fn new_init_db() -> Result<Self> {
        let db_cipher = XChaCha20Poly1305Cipher::with_key(db_key());
        if tokio::fs::try_exists(DB_PATH).await.unwrap() {
            bail!("Database already exists");
        }
        let client_id = terminal::prompt("Plaid Client ID").unwrap();
        let secret = terminal::prompt("Plaid Secret").unwrap();
        let db = DatabaseV1::new(DbPlaidAuth::new(client_id, secret));
        Ok(Self::_new(db, db_cipher))
    }

    pub async fn new_load_db() -> Result<Self> {
        let db_cipher = XChaCha20Poly1305Cipher::with_key(db_key());
        let db = db::load(&Path::new(DB_PATH), &db_cipher)
            .await
            .context("Failed to load database")?
            .ok_or_else(|| anyhow!("Database file not found"))?;
        Ok(Self::_new(db, db_cipher))
    }

    fn _new(db: DatabaseV1, db_cipher: XChaCha20Poly1305Cipher) -> Self {
        let plaid_api = plaid_api::Plaid::new(db.plaid_auth.to_api_auth());
        Self {
            db,
            db_cipher,
            plaid_api,
        }
    }

    pub async fn save_db(self) -> Result<()> {
        db::save(self.db, &Path::new(DB_PATH), &self.db_cipher)
            .await
            .context("Failed to save database")?;
        Ok(())
    }

    pub async fn main_init(&self) -> Result<()> {
        // Test the API connection
        plaid_api::test_connection(&self.plaid_api)
            .await
            .context("Plaid API connection failed")?;
        Ok(())
    }

    pub async fn main_add_connection(&mut self) -> Result<()> {
        let name = terminal::prompt("Enter a name for the new connection").unwrap();
        println!();
        let access_token = plaid_api::link_new_account(&self.plaid_api).await.unwrap();
        let accounts = plaid_api::get_accounts(&self.plaid_api, &access_token)
            .await
            .unwrap();
        println!();
        println!("Found {} accounts", accounts.len());
        let accounts = accounts
            .enumerate()
            .map(|(index, account)| {
                let (id, account) = account?;
                Ok(prompt_add_account(index, id, account)?)
            })
            .collect::<Result<_>>()?;
        let connection = BankConnection::new(name, access_token, accounts);
        println!();
        println!("{}", style_header("Adding connection:"));
        print_connection(&BulletPointPrinter::new_stdout(), &connection);
        self.db.bank_connections.push(connection);
        Ok(())
    }

    pub async fn main_list_connections(&self) -> Result<()> {
        println!("{}", style_header("Connections:"));
        if self.db.bank_connections.is_empty() {
            println!("(none)");
        } else {
            let printer = BulletPointPrinter::new_stdout();
            for connection in &self.db.bank_connections {
                print_connection(&printer, connection);
            }
        }
        Ok(())
    }

    pub async fn main_sync(&mut self) -> Result<()> {
        println!("{}", style_header("Syncing connections:"));
        let printer = BulletPointPrinter::new_stdout();
        for connection in &mut self.db.bank_connections {
            Self::sync_connection(&self.plaid_api, connection, &printer).await?;
        }
        Ok(())
    }

    async fn sync_connection(
        plaid_api: &plaid_api::Plaid,
        bank_connection: &mut BankConnection,
        printer: &BulletPointPrinter<impl LineWriter + Clone>,
    ) -> Result<()> {
        printer.print_item(style_connection(bank_connection));
        let printer = printer.indent();

        let transactions =
            plaid_api::get_transactions(plaid_api, &bank_connection.access_token()).await?;

        let mut num_added: HashMap<AccountId, u64> = bank_connection
            .accounts()
            .map(|(id, _)| (id.clone(), 0))
            .collect();
        let mut num_verified = num_added.clone();
        for transaction in transactions {
            let account = bank_connection
                .account_mut(&transaction.account_id)
                .ok_or_else(|| {
                    anyhow!(
                        "Found transaction for account {:?} that we don't have in our database",
                        transaction.account_id,
                    )
                })?;
            if let Some(account) = &mut account.account {
                let transaction_id = transaction.transaction_id.clone();
                let add_or_verify_result = account
                    .add_or_verify_transaction(transaction.transaction_id, transaction.transaction);
                match add_or_verify_result {
                    AddOrVerifyResult::Added => {
                        *num_added.get_mut(&transaction.account_id).unwrap() += 1;
                    }
                    AddOrVerifyResult::ExistsAndMatches => {
                        *num_verified.get_mut(&transaction.account_id).unwrap() += 1;
                    }
                    AddOrVerifyResult::ExistsAndDoesntMatch => {
                        bail!("Transaction {transaction_id:?} already exists but doesn't match",);
                    }
                }
            } else {
                *num_added.get_mut(&transaction.account_id).unwrap() += 1;
            }
        }

        for account in bank_connection.accounts() {
            printer.print_item(style_account(&account.1));
            let printer = printer.indent();
            if account.1.is_connected() {
                printer.print_item(
                    style(format!("Added: {}", num_added.get(&account.0).unwrap())).italic(),
                );
                printer.print_item(
                    style(format!(
                        "Verified: {}",
                        num_verified.get(&account.0).unwrap()
                    ))
                    .italic(),
                );
            } else {
                printer.print_item(
                    style(format!(
                        "Transactions: {}",
                        num_added.get(&account.0).unwrap()
                    ))
                    .italic()
                    .strikethrough(),
                );
            }
        }

        Ok(())
    }

    pub async fn main_list_transactions(&mut self) -> Result<()> {
        println!("{}", style_header("Transactions:"));
        let printer = BulletPointPrinter::new_stdout();
        for connection in &self.db.bank_connections {
            printer.print_item(style_connection(connection));
            let printer = printer.indent();
            for account in connection.accounts() {
                if let Some(connected_account) = &account.1.account {
                    printer.print_item(style_account(account.1));
                    let printer = printer.indent();
                    let transactions = &connected_account.transactions;
                    if transactions.is_empty() {
                        printer.print_item(style("(none)").italic());
                    } else {
                        for transaction in connected_account.transactions.iter_all_sorted_by_date()
                        {
                            print_transaction(&printer, &transaction.1);
                        }
                    }
                } else {
                    printer.print_item(style_account(&account.1).strikethrough());
                }
            }
        }
        Ok(())
    }

    pub async fn main_export_all_transactions(&mut self) -> Result<()> {
        let all_transactions = self.db.bank_connections.iter().flat_map(|c| {
            c.accounts().flat_map(|account| {
                account.1.account.iter().flat_map(|account| {
                    account.transactions.iter_all_sorted_by_date().map(
                        move |(transaction_id, transaction)| {
                            (&account.beancount_account_info, transaction_id, transaction)
                        },
                    )
                })
            })
        });
        export_transactions(all_transactions)?;
        Ok(())
    }

    pub async fn main_export_new_transactions(&mut self) -> Result<()> {
        let new_transactions = self.db.bank_connections.iter_mut().flat_map(|c| {
            c.accounts_mut().flat_map(|account| {
                account.1.account.iter_mut().flat_map(|account| {
                    account.transactions.iter_new_sorted_by_date_mut().map(
                        |(transaction_id, transaction)| {
                            transaction.mark_as_exported();
                            (
                                &account.beancount_account_info,
                                transaction_id,
                                &*transaction,
                            )
                        },
                    )
                })
            })
        });
        export_transactions(new_transactions)?;
        Ok(())
    }
}

fn prompt_add_account(
    index: usize,
    account_id: AccountId,
    plaid_account_info: PlaidAccountInfo,
) -> Result<(AccountId, Account)> {
    print_found_account(index, &plaid_account_info);
    println!();
    let prompt = "Add account";
    if terminal::prompt_yes_no(&prompt)? {
        let beancount_account_info = prompt_beancount_account_info()?;
        Ok((
            account_id,
            Account::new_connected(plaid_account_info, beancount_account_info),
        ))
    } else {
        Ok((account_id, Account::new_unconnected(plaid_account_info)))
    }
}

fn print_found_account(index: usize, plaid_account_info: &PlaidAccountInfo) {
    println!();
    println!("{}", style_header(&format!("Account {}:", index + 1)));
    print!("{}", style("Account name: ").bold());
    print!("{}", style(&plaid_account_info.name).magenta());
    if let Some(official_name) = &plaid_account_info.official_name {
        if *official_name != plaid_account_info.name {
            print!(" ({})", style(official_name).magenta());
        }
    }
    println!();
    if let Some(mask) = &plaid_account_info.mask {
        print!("{}", style("Account number: ").bold());
        println!("{}", style_mask(mask));
    }
    print!("{}", style("Type: ").bold());
    print!("{}", style(&plaid_account_info.type_).cyan());
    if let Some(subtype) = &plaid_account_info.subtype {
        println!(" / {}", style(subtype).cyan());
    }
}

fn prompt_beancount_account_info() -> Result<BeancountAccountInfo> {
    const PROMPT: &str = "Beancount account name";
    let mut name = terminal::prompt(PROMPT)?;
    loop {
        match parse_beancount_account_info(&name) {
            Ok(info) => return Ok(info),
            Err(err) => {
                println!("{}", style(err).red().bold());
                name = terminal::prompt(PROMPT)?;
            }
        }
    }
}

fn parse_beancount_account_info(name: &str) -> Result<BeancountAccountInfo, &'static str> {
    let mut parts = name.split(':');
    let ty = parts
        .next()
        .expect("There should always be at least one part to the split");
    let ty = match ty {
        "Assets" => AccountType::Assets,
        "Liabilities" => AccountType::Liabilities,
        "Equity" => AccountType::Equity,
        "Income" => AccountType::Income,
        "Expenses" => AccountType::Expenses,
        _ => return Err(
            "Account must start with one of: Assets:, Liabilities:, Equity:, Income:, Expenses:",
        ),
    };
    Ok(BeancountAccountInfo {
        ty,
        name_parts: parts.map(|v| v.to_string()).collect(),
    })
}

fn print_accounts<'a, 'b>(
    printer: &BulletPointPrinter<impl LineWriter + Clone>,
    accounts: impl Iterator<Item = (&'a AccountId, &'b Account)>,
) {
    for account in accounts {
        printer.print_item(style_account(account.1));
    }
}

fn print_connection(
    printer: &BulletPointPrinter<impl LineWriter + Clone>,
    connection: &BankConnection,
) {
    printer.print_item(style_connection(connection));
    print_accounts(&printer.indent(), connection.accounts());
}

fn print_transaction(
    printer: &BulletPointPrinter<impl LineWriter + Clone>,
    transaction: &Transaction,
) {
    let transaction_description = transaction
        .transaction
        .original_description
        .as_ref()
        .map(|desc| format!(" \"{desc}\""))
        .unwrap_or_else(|| "".to_string());
    let merchant_name = transaction
        .transaction
        .merchant_name
        .as_ref()
        .map(|name| format!(" {name}"))
        .unwrap_or_else(|| "".to_string());
    // TODO Use plaid_categories.csv (it's in the repo, this is the official list from plaid) to display categories
    let category = transaction
        .transaction
        .category
        .as_ref()
        .map(|cat| format!(" [{}.{}]", cat.primary, cat.detailed))
        .unwrap_or_else(|| "".to_string());
    let date = if let Some(authorized_date) = transaction.transaction.authorized_date {
        if authorized_date != transaction.transaction.posted_date {
            format!(
                "{} (posted: {})",
                authorized_date.format("%Y-%m-%d"),
                transaction.transaction.posted_date.format("%Y-%m-%d")
            )
        } else {
            transaction
                .transaction
                .posted_date
                .format("%Y-%m-%d")
                .to_string()
        }
    } else {
        transaction
            .transaction
            .posted_date
            .format("%Y-%m-%d")
            .to_string()
    };
    printer.print_item(style_transaction(&format!(
        "{} {}{}{}{} {}",
        pad_str(&style_date(&date).to_string(), 10, Alignment::Left, None),
        pad_str(
            &style_amount(&transaction.transaction.amount).to_string(),
            15,
            Alignment::Right,
            None
        ),
        style_transaction_description(&transaction_description),
        style_merchant_name(&merchant_name),
        style_category(&category),
        if transaction.already_exported {
            style("[exported]").dim()
        } else {
            style("[new]").dim()
        },
    )));
    let printer = printer.indent();
    if let Some(location) = &transaction.transaction.location {
        if location != "{}" {
            printer.print_item(style(format!("Location: {}", location)).dim());
        }
    }
    if let Some(website) = &transaction.transaction.associated_website {
        printer.print_item(style(format!("Website: {}", website)).dim());
    }
    if let Some(check_number) = &transaction.transaction.check_number {
        printer.print_item(style(format!("Check number: {}", check_number)).dim());
    }
}

fn style_header(header: &str) -> StyledObject<&str> {
    style(header).bold().underlined()
}

fn style_connection(connection: &BankConnection) -> StyledObject<&str> {
    style(connection.name()).cyan().bold()
}

fn style_account(account: &Account) -> StyledObject<String> {
    let mut account_info = account.plaid_account_info.name.clone();
    if let Some(mask) = &account.plaid_account_info.mask {
        account_info.push_str(" ");
        account_info.push_str(&style_mask(&mask).to_string());
    }
    if let Some(connected_account) = &account.account {
        style(format!(
            "{} {}",
            account_info,
            style(format!(
                "[{}]",
                connected_account.beancount_account_info.beancount_name()
            ))
            .italic(),
        ))
        .magenta()
    } else {
        style(format!(
            "{} {}",
            account_info,
            style(format!("[account not connected]")).italic(),
        ))
        .magenta()
        .strikethrough()
    }
}

fn style_transaction(transaction: &str) -> StyledObject<&str> {
    style(transaction).italic()
}

fn style_date(date: &str) -> StyledObject<&str> {
    style(date)
}

fn style_amount(amount: &Amount) -> StyledObject<String> {
    let result = style(format!(
        "{} {}",
        amount.amount,
        amount.iso_currency_code.as_deref().unwrap_or("???")
    ))
    .bold();
    if amount.amount < Decimal::ZERO {
        result.red()
    } else {
        result.green()
    }
}

fn style_transaction_description(description: &str) -> StyledObject<&str> {
    style(description).blue()
}

fn style_merchant_name(merchant_name: &str) -> StyledObject<&str> {
    style(merchant_name).yellow()
}

fn style_category(category: &str) -> StyledObject<&str> {
    style(category).magenta()
}

fn style_mask(mask: &str) -> StyledObject<String> {
    style(format!("***{mask}")).italic()
}
