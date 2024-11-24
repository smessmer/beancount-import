use serde::{Deserialize, Serialize};

use super::Transactions;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct AccountId(pub String);

impl AccountId {
    pub fn new(id: String) -> Self {
        Self(id)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct AccountInfo {
    pub id: AccountId,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Account {
    pub account_info: AccountInfo,
    pub transactions: Transactions,
}

impl Account {
    pub fn new(account_info: AccountInfo) -> Self {
        Self {
            account_info,
            transactions: Transactions::new_empty(),
        }
    }
}
