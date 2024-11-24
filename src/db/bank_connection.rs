use serde::{Deserialize, Serialize};

use crate::plaid_api;

use super::DbTransactions;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DbBankConnection {
    name: String,
    access_token: DbAccessToken,
    accounts: Vec<DbAccount>,
}

impl DbBankConnection {
    pub fn new(name: String, access_token: DbAccessToken, accounts: Vec<DbAccount>) -> Self {
        Self {
            name,
            access_token,
            accounts,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn access_token(&self) -> &DbAccessToken {
        &self.access_token
    }

    pub fn accounts(&self) -> &[DbAccount] {
        &self.accounts
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DbAccessToken(String);

impl DbAccessToken {
    pub fn new(access_token: String) -> Self {
        Self(access_token)
    }

    pub fn to_plaid_api(&self) -> plaid_api::AccessToken {
        plaid_api::AccessToken::new(self.0.clone())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DbAccount {
    pub account_id: String,
    pub name: String,
    pub transactions: DbTransactions,
}

impl DbAccount {
    pub fn new(account: plaid_api::AccountInfo) -> Self {
        Self {
            account_id: account.id.0,
            name: account.name,
            transactions: DbTransactions::new_empty(),
        }
    }
}
