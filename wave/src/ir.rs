use std::{
    collections::{HashMap, HashSet},
    iter::Sum,
    ops::{Add, AddAssign, Neg, Sub},
};

use chrono::NaiveDate;
use rust_decimal::{prelude::Zero as _, Decimal};

pub const LEDGER_CURRENCY: &str = "USD";
pub const LEDGER_CURRENCY_SYMBOL: &str = "$";

#[derive(Debug, Clone)]
pub struct Ledger {
    pub ledger_name: String,
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Amount {
    pub in_account_currency: Decimal,
    pub in_ledger_currency: Decimal,
}

impl Amount {
    pub fn zero() -> Amount {
        Amount {
            in_account_currency: Decimal::zero(),
            in_ledger_currency: Decimal::zero(),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.in_account_currency.is_zero() && self.in_ledger_currency.is_zero()
    }
}

impl Add<Amount> for Amount {
    type Output = Amount;

    fn add(self, other: Amount) -> Amount {
        Amount {
            in_account_currency: self.in_account_currency + other.in_account_currency,
            in_ledger_currency: self.in_ledger_currency + other.in_ledger_currency,
        }
    }
}

impl AddAssign<Amount> for Amount {
    fn add_assign(&mut self, other: Amount) {
        self.in_account_currency += other.in_account_currency;
        self.in_ledger_currency += other.in_ledger_currency;
    }
}

impl Sum for Amount {
    fn sum<I: Iterator<Item = Amount>>(iter: I) -> Amount {
        iter.fold(Amount::zero(), |acc, amount| acc + amount)
    }
}

impl Sub<Amount> for Amount {
    type Output = Amount;

    fn sub(self, other: Amount) -> Amount {
        Amount {
            in_account_currency: self.in_account_currency - other.in_account_currency,
            in_ledger_currency: self.in_ledger_currency - other.in_ledger_currency,
        }
    }
}

impl Neg for Amount {
    type Output = Amount;

    fn neg(self) -> Amount {
        Amount {
            in_account_currency: -self.in_account_currency,
            in_ledger_currency: -self.in_ledger_currency,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AccountBalance {
    pub start_balance: Amount,
    pub end_balance: Amount,
    pub account_currency: String,
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
            .sum::<Amount>()
            .is_zero()
    }
}

#[derive(Debug, Clone)]
pub struct Posting {
    pub account_name: String,
    pub amount: Amount,
}
