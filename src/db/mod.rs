use serde::{Deserialize, Serialize};

mod access_token;
mod account;
mod bank_connection;
mod crypto;
mod database;
mod file;
mod plaid_auth;
mod transactions;

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, Eq, Debug))]
pub enum Database {
    V1(DatabaseV1),
}

pub use access_token::AccessToken;
pub use account::{Account, AccountId, AccountInfo};
pub use bank_connection::BankConnection;
pub use crypto::{Cipher, XChaCha20Poly1305Cipher};
pub use database::DatabaseV1;
pub use file::{load, save};
pub use plaid_auth::DbPlaidAuth;
pub use transactions::{Amount, Transaction, TransactionCategory, Transactions};
