use serde::{Deserialize, Serialize};

use super::{account::Account, AccessToken};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct BankConnection {
    name: String,
    access_token: AccessToken,
    accounts: Vec<Account>,
}

impl BankConnection {
    pub fn new(name: String, access_token: AccessToken, accounts: Vec<Account>) -> Self {
        Self {
            name,
            access_token,
            accounts,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn access_token(&self) -> &AccessToken {
        &self.access_token
    }

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }
}
