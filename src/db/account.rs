use serde::{Deserialize, Serialize};

use super::{transactions::AddOrVerifyResult, Transaction, TransactionId, Transactions};

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

    pub fn add_or_verify_transaction(
        &mut self,
        transaction_id: TransactionId,
        transaction: Transaction,
    ) -> AddOrVerifyResult {
        self.transactions.add_or_verify(transaction_id, transaction)
    }
}
