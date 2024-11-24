use anyhow::{anyhow, bail, Context as _, Result};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use std::path::Path;

use crate::args::{self, Args, Command};

use super::db::{self, Cipher, DatabaseV1, DbBankConnection, DbPlaidAuth, XChaCha20Poly1305Cipher};
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
        let client_id = dialoguer::Input::new()
            .with_prompt("Plaid Client ID")
            .interact()
            .unwrap();
        let secret = dialoguer::Input::new()
            .with_prompt("Plaid Secret")
            .interact()
            .unwrap();
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
        let name = dialoguer::Input::new()
            .with_prompt("Enter a name for the new connection")
            .interact()
            .unwrap();
        let access_token = plaid_api::link_new_account(&self.plaid_api).await.unwrap();
        println!("Access token: {:?}", access_token);
        let accounts = plaid_api::get_accounts(&self.plaid_api, &access_token)
            .await
            .unwrap();
        println!("Accounts: {:?}", accounts);
        self.db.bank_connections.push(DbBankConnection::new(
            name,
            access_token.to_db(),
            accounts.into_iter().map(Into::into).collect(),
        ));
        Ok(())
    }

    pub async fn main_sync(&mut self) -> Result<()> {
        // TODO No clone of bank_connections
        for connection in self.db.bank_connections.clone() {
            self.sync_connection(&connection).await?;
        }
        Ok(())
    }

    async fn sync_connection(&mut self, connection: &DbBankConnection) -> Result<()> {
        println!("Syncing connection: {}", connection.name());
        for account in connection.accounts() {
            println!(" * {}", account.name);
        }
        let transactions =
            plaid_api::get_transactions(&self.plaid_api, &connection.access_token().to_plaid_api())
                .await;

        println!("Transactions: {:?}", transactions);

        Ok(())
    }
}
