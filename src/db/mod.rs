use serde::{Deserialize, Serialize};

mod bank_connection;
mod crypto;
mod database;
mod file;
mod plaid_auth;

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, Eq, Debug))]
pub enum Database {
    V1(DatabaseV1),
}

pub use bank_connection::DbBankConnection;
pub use crypto::{Cipher, XChaCha20Poly1305Cipher};
pub use database::DatabaseV1;
pub use file::{load, load_or_else, save};
pub use plaid_auth::DbPlaidAuth;
