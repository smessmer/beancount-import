use chrono::NaiveDate;
use common_macros::hash_map;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
};

#[must_use]
pub enum AddOrVerifyResult {
    Added,
    ExistsAndMatches,
    ExistsAndDoesntMatch,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Transactions {
    transactions: HashMap<TransactionId, Transaction>,
}

impl Transactions {
    pub fn new_empty() -> Self {
        Self {
            transactions: hash_map![],
        }
    }

    pub fn add_or_verify(
        &mut self,
        id: TransactionId,
        transaction: Transaction,
    ) -> AddOrVerifyResult {
        match self.transactions.entry(id.clone()) {
            Entry::Occupied(entry) => {
                if entry.get() == &transaction {
                    AddOrVerifyResult::ExistsAndMatches
                } else {
                    AddOrVerifyResult::ExistsAndDoesntMatch
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(transaction);
                AddOrVerifyResult::Added
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&TransactionId, &Transaction)> {
        self.transactions.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransactionId(pub String);

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Amount {
    #[serde(with = "rust_decimal::serde::str")]
    pub amount: Decimal,
    pub iso_currency_code: Option<String>,
}

impl Debug for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}",
            self.amount,
            self.iso_currency_code
                .as_ref()
                .map(|a| a.as_str())
                .unwrap_or("[UKN]")
        )
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TransactionCategory {
    pub primary: String,
    pub detailed: String,
}

impl Debug for TransactionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.primary, self.detailed)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub merchant_name: Option<String>,
    pub description: Option<String>,
    pub date: NaiveDate,
    pub category: Option<TransactionCategory>,
    pub amount: Amount,
}
