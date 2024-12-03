use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct Ledger {
    pub dates: Dates,
    pub account_balances: HashMap<String, AccountBalance>,
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
pub struct AccountBalance {
    pub start_balance: Decimal,
    pub end_balance: Decimal,
}

#[derive(Debug, Clone, Copy)]
pub struct Dates {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
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
