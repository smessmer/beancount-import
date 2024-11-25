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
use crate::terminal::{self, BulletPointPrinter};

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
        Command::Export => cli.main_export_transactions().await?,
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
        let accounts = accounts
            .map(|(id, account)| prompt_add_account(id, account))
            .collect::<Result<_>>()?;
        let connection = BankConnection::new(name, access_token, accounts);
        println!();
        println!("{}", style_header("Adding connection:"));
        print_connection(&BulletPointPrinter::new(), &connection);
        self.db.bank_connections.push(connection);
        Ok(())
    }

    pub async fn main_list_connections(&self) -> Result<()> {
        println!("{}", style_header("Connections:"));
        if self.db.bank_connections.is_empty() {
            println!("(none)");
        } else {
            let printer = BulletPointPrinter::new();
            for connection in &self.db.bank_connections {
                print_connection(&printer, connection);
            }
        }
        Ok(())
    }

    pub async fn main_sync(&mut self) -> Result<()> {
        println!("{}", style_header("Syncing connections:"));
        let printer = BulletPointPrinter::new();
        for connection in &mut self.db.bank_connections {
            Self::sync_connection(&self.plaid_api, connection, &printer).await?;
        }
        Ok(())
    }

    async fn sync_connection(
        plaid_api: &plaid_api::Plaid,
        bank_connection: &mut BankConnection,
        printer: &BulletPointPrinter,
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
        let printer = BulletPointPrinter::new();
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
                        for transaction in connected_account.transactions.iter() {
                            print_transaction(&printer, transaction.1);
                        }
                    }
                } else {
                    printer.print_item(style_account(&account.1).strikethrough());
                }
            }
        }
        Ok(())
    }

    pub async fn main_export_transactions(&mut self) -> Result<()> {
        export_transactions(self.db.bank_connections.iter().flat_map(|c| {
            c.accounts().flat_map(|a| {
                a.1.account
                    .iter()
                    .flat_map(|account| account.transactions.iter())
            })
        }))?;
        Ok(())
    }
}

fn prompt_add_account(
    account_id: AccountId,
    plaid_account_info: PlaidAccountInfo,
) -> Result<(AccountId, Account)> {
    let prompt = format!("Add account {}", plaid_account_info.name);
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
    printer: &BulletPointPrinter,
    accounts: impl Iterator<Item = (&'a AccountId, &'b Account)>,
) {
    for account in accounts {
        printer.print_item(style_account(account.1));
    }
}

fn print_connection(printer: &BulletPointPrinter, connection: &BankConnection) {
    printer.print_item(style_connection(connection));
    print_accounts(&printer.indent(), connection.accounts());
}

fn print_transaction(printer: &BulletPointPrinter, transaction: &Transaction) {
    let transaction_description = transaction
        .description
        .as_ref()
        .map(|desc| format!(" \"{desc}\""))
        .unwrap_or_else(|| "".to_string());
    let merchant_name = transaction
        .merchant_name
        .as_ref()
        .map(|name| format!(" {name}"))
        .unwrap_or_else(|| "".to_string());
    let category = transaction
        .category
        .as_ref()
        .map(|cat| format!(" [{}.{}]", cat.primary, cat.detailed))
        .unwrap_or_else(|| "".to_string());
    printer.print_item(style_transaction(&format!(
        "{} {}{}{}{}",
        pad_str(
            &style_date(&transaction.date).to_string(),
            10,
            Alignment::Left,
            None
        ),
        pad_str(
            &style_amount(&transaction.amount).to_string(),
            15,
            Alignment::Right,
            None
        ),
        style_transaction_description(&transaction_description),
        style_merchant_name(&merchant_name),
        style_category(&category),
    )));
}

fn style_header(header: &str) -> StyledObject<&str> {
    style(header).bold().underlined()
}

fn style_connection(connection: &BankConnection) -> StyledObject<&str> {
    style(connection.name()).cyan().bold()
}

fn style_account(account: &Account) -> StyledObject<String> {
    if let Some(connected_account) = &account.account {
        style(format!(
            "{} {}",
            account.plaid_account_info.name,
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
            account.plaid_account_info.name,
            style(format!("[account not connected]")).italic(),
        ))
        .magenta()
        .strikethrough()
    }
}

fn style_transaction(transaction: &str) -> StyledObject<&str> {
    style(transaction).italic()
}

fn style_date(date: &chrono::NaiveDate) -> StyledObject<String> {
    style(date.format("%Y-%m-%d").to_string())
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
