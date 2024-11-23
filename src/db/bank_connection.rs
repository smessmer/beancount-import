use serde::{Deserialize, Serialize};

use crate::plaid_api;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DbBankConnection {
    pub access_token: String,
    pub accounts: Vec<DbAccount>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DbAccount {
    pub account_id: String,
    pub name: String,
}

impl From<plaid_api::AccountInfo> for DbAccount {
    fn from(account: plaid_api::AccountInfo) -> Self {
        Self {
            account_id: account.id.0,
            name: account.name,
        }
    }
}

impl From<DbAccount> for plaid_api::AccountInfo {
    fn from(account: DbAccount) -> Self {
        Self {
            id: plaid_api::AccountId(account.account_id),
            name: account.name,
        }
    }
}
