use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{Transaction, TransactionId, Transactions};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountId(pub String);

impl AccountId {
    pub fn new(id: String) -> Self {
        Self(id)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct AccountInfo {
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

    pub fn add_transaction(
        &mut self,
        transaction_id: TransactionId,
        transaction: Transaction,
    ) -> Result<()> {
        self.transactions.add(transaction_id, transaction)
    }
}
