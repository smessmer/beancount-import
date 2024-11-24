use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Transactions {
    transactions: Vec<Transaction>,
}

impl Transactions {
    pub fn new_empty() -> Self {
        Self {
            transactions: vec![],
        }
    }

    pub fn add(&mut self, transaction: Transaction) {
        self.transactions.push(transaction);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Transaction> {
        self.transactions.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct TransactionId(pub String);

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
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

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct TransactionCategory {
    pub primary: String,
    pub detailed: String,
}

impl Debug for TransactionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.primary, self.detailed,)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Transaction {
    pub id: TransactionId,
    pub merchant_name: Option<String>,
    pub description: Option<String>,
    pub date: NaiveDate,
    pub category: Option<TransactionCategory>,
    pub amount: Amount,
}
