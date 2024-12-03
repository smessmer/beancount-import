use std::collections::HashSet;

use chrono::NaiveDate;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct Ledger {
    pub transactions: Vec<Transaction>,
}

impl Ledger {
    pub fn account_names(&self) -> HashSet<&str> {
        self.transactions
            .iter()
            .flat_map(|transaction| {
                transaction
                    .postings
                    .iter()
                    .map(|posting| posting.account_name.as_str())
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub date: NaiveDate,
    pub description: String,
    pub postings: Vec<Posting>,
}

impl Transaction {
    pub fn is_balanced(&self) -> bool {
        self.postings
            .iter()
            .map(|posting| posting.amount)
            .sum::<Decimal>()
            .is_zero()
    }
}

#[derive(Debug, Clone)]
pub struct Posting {
    pub account_name: String,
    pub amount: Decimal,
}
