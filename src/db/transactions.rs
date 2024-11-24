use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Transactions {}

impl Transactions {
    pub fn new_empty() -> Self {
        Self {}
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Amount {
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
    pub merchant_name: Option<String>,
    pub description: Option<String>,
    pub date: Option<NaiveDate>,
    pub category: Option<TransactionCategory>,
    pub amount: Amount,
}
