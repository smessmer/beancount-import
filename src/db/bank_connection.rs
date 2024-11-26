use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{account::Account, AccessToken, AccountId};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct BankConnection {
    name: String,
    access_token: AccessToken,
    accounts: HashMap<AccountId, Account>,
}

impl BankConnection {
    pub fn new(
        name: String,
        access_token: AccessToken,
        accounts: HashMap<AccountId, Account>,
    ) -> Self {
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

    pub fn accounts(&self) -> impl Iterator<Item = (&AccountId, &Account)> {
        self.accounts.iter()
    }

    pub fn accounts_mut(&mut self) -> impl Iterator<Item = (&AccountId, &mut Account)> {
        self.accounts.iter_mut()
    }

    pub fn account(&self, account_id: &AccountId) -> Option<&Account> {
        self.accounts.get(account_id)
    }

    pub fn account_mut(&mut self, account_id: &AccountId) -> Option<&mut Account> {
        self.accounts.get_mut(account_id)
    }
}
