use anyhow::Result;
use std::path::Path;

use beancount_plaid::db::{self, Cipher, DatabaseV1, DbBankConnection, XChaCha20Poly1305Cipher};
use rand::{rngs::StdRng, RngCore, SeedableRng};

// TODO Configurable DB Location
const DB_PATH: &str = "beancount_plaid.db";

// TODO Configurable encryption key
fn db_key() -> chacha20poly1305::Key {
    let mut rng = StdRng::seed_from_u64(1);
    let mut key_bytes = [0; 32];
    rng.fill_bytes(&mut key_bytes);
    key_bytes.into()
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let db_cipher = XChaCha20Poly1305Cipher::with_key(db_key());
    let mut db = db::load_or_empty(&Path::new(DB_PATH), &db_cipher).await?;

    let client = beancount_plaid::plaid_api::Plaid::new();
    let access_token = beancount_plaid::plaid_api::link_new_account(&client)
        .await
        .unwrap();
    println!("Access token: {:?}", access_token);
    let accounts = beancount_plaid::plaid_api::get_accounts(&client, &access_token)
        .await
        .unwrap();
    println!("Accounts: {:?}", accounts);
    let transactions = beancount_plaid::plaid_api::get_transactions(&client, &access_token).await;
    println!("Transactions: {:?}", transactions);

    db.bank_connections.push(DbBankConnection {
        access_token: access_token.access_token,
        accounts: accounts.into_iter().map(Into::into).collect(),
    });

    db::save(db, &Path::new(DB_PATH), &db_cipher).await?;

    Ok(())
}
