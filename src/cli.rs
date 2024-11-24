use anyhow::{anyhow, bail, Context as _, Result};
use console::{pad_str, style, Alignment, StyledObject};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use rust_decimal::Decimal;
use std::path::Path;

use crate::args::{Args, Command};
use crate::db::{Account, AccountId, AccountInfo, Amount, Transaction};
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
        let connection = BankConnection::new(
            name,
            access_token,
            accounts
                .map(|(id, account)| (id, Account::new(account)))
                .collect(),
        );
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
        // TODO No clone of bank_connections
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
        print_connection(printer, bank_connection);
        let transactions =
            plaid_api::get_transactions(plaid_api, &bank_connection.access_token()).await?;

        // TODO Don't just add transactions, look for existing ones to overwrite
        let num_transactions = transactions.len();
        for transaction in transactions {
            bank_connection
                .account_mut(&transaction.account_id)
                .ok_or_else(|| {
                    anyhow!(
                        "Found transaction for account {:?} that we don't have in our database",
                        transaction.account_id,
                    )
                })?
                .add_transaction(transaction.transaction_id, transaction.transaction)?;
        }
        // TODO Show added transactions per account
        println!("Added {num_transactions} transactions");

        Ok(())
    }

    pub async fn main_list_transactions(&mut self) -> Result<()> {
        println!("{}", style_header("Transactions:"));
        let printer = BulletPointPrinter::new();
        for connection in &self.db.bank_connections {
            printer.print_item(style_connection(connection));
            let printer = printer.indent();
            for account in connection.accounts() {
                printer.print_item(style_account(&account.1.account_info));
                let printer = printer.indent();
                let transactions = &account.1.transactions;
                if transactions.is_empty() {
                    printer.print_item(style("(none)").italic());
                } else {
                    for transaction in account.1.transactions.iter() {
                        print_transaction(&printer, transaction.1);
                    }
                }
            }
        }
        Ok(())
    }
}

fn print_accounts<'a, 'b>(
    printer: &BulletPointPrinter,
    accounts: impl Iterator<Item = (&'a AccountId, &'b Account)>,
) {
    for account in accounts {
        printer.print_item(style_account(&account.1.account_info));
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

fn style_account(account: &AccountInfo) -> StyledObject<&str> {
    style(account.name.as_str()).magenta()
}

fn style_transaction(transaction: &str) -> StyledObject<&str> {
    style(transaction).italic()
}

fn style_date(date: &chrono::NaiveDate) -> StyledObject<String> {
    // TODO
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
