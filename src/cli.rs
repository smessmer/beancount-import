use anyhow::{anyhow, bail, Context as _, Result};
use console::{style, StyledObject};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use std::path::Path;

use crate::args::{Args, Command};
use crate::db::{Account, AccountInfo};
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
            .await?
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
        db::save(self.db, &Path::new(DB_PATH), &self.db_cipher).await?;
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
            accounts.into_iter().map(Account::new).collect(),
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
        for connection in self.db.bank_connections.clone() {
            self.sync_connection(&connection, &printer).await?;
        }
        Ok(())
    }

    async fn sync_connection(
        &mut self,
        connection: &BankConnection,
        printer: &BulletPointPrinter,
    ) -> Result<()> {
        print_connection(printer, connection);
        let transactions =
            plaid_api::get_transactions(&self.plaid_api, &connection.access_token()).await?;

        // TODO Remove println, instead add to db and print number added
        println!("Transactions: {:?}", transactions);

        Ok(())
    }
}

fn print_accounts(printer: &BulletPointPrinter, accounts: &[Account]) {
    for account in accounts {
        printer.print_item(style_account(&account.account_info));
    }
}

fn print_connection(printer: &BulletPointPrinter, connection: &BankConnection) {
    printer.print_item(style_connection(connection));
    print_accounts(&printer.indent(), connection.accounts());
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
