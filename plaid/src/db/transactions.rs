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
    ExistsAndDoesntMatch {
        existing_value: Transaction,
        new_value: Transaction,
    },
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
                if entry.get().transaction == transaction.transaction {
                    AddOrVerifyResult::ExistsAndMatches
                } else {
                    AddOrVerifyResult::ExistsAndDoesntMatch {
                        existing_value: entry.get().clone(),
                        new_value: transaction,
                    }
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(transaction);
                AddOrVerifyResult::Added
            }
        }
    }

    pub fn iter_all_sorted_by_date(&self) -> impl Iterator<Item = (&TransactionId, &Transaction)> {
        sorted_by_date(self.transactions.iter())
    }

    pub fn iter_all_sorted_by_date_mut(
        &mut self,
    ) -> impl Iterator<Item = (&TransactionId, &mut Transaction)> {
        sorted_by_date_mut(self.transactions.iter_mut())
    }

    pub fn iter_new_sorted_by_date_mut(
        &mut self,
    ) -> impl Iterator<Item = (&TransactionId, &mut Transaction)> {
        sorted_by_date_mut(
            self.transactions
                .iter_mut()
                .filter(|(_, t)| !t.already_exported),
        )
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}

fn sorted_by_date<'a, 'b>(
    transactions: impl Iterator<Item = (&'a TransactionId, &'b Transaction)>,
) -> impl Iterator<Item = (&'a TransactionId, &'b Transaction)> {
    let mut transactions: Vec<(&TransactionId, &Transaction)> = transactions.collect();
    transactions.sort_by_key(|(_, t)| t.transaction.date());
    transactions.into_iter()
}

fn sorted_by_date_mut<'a, 'b>(
    transactions: impl Iterator<Item = (&'a TransactionId, &'b mut Transaction)>,
) -> impl Iterator<Item = (&'a TransactionId, &'b mut Transaction)> {
    let mut transactions: Vec<(&TransactionId, &mut Transaction)> = transactions.collect();
    transactions.sort_by_key(|(_, t)| t.transaction.date());
    transactions.into_iter()
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

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
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
    pub transaction: TransactionInfo,
    pub already_exported: bool,
}

impl Transaction {
    pub fn new(transaction: TransactionInfo) -> Self {
        Self {
            transaction,
            already_exported: false,
        }
    }

    pub fn mark_as_exported(&mut self) {
        self.already_exported = true;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TransactionInfo {
    pub posted_date: NaiveDate,
    pub authorized_date: Option<NaiveDate>,
    pub category: Option<TransactionCategory>,

    /// Positive amounts mean money into asset accounts or payments for credit card purchases
    /// Negative amounts mean money out of asset accounts or credit card purchases
    /// This aligns with beancount but is the opposite of the Plaid API, which uses positive amounts for money out.
    pub amount: Amount,

    pub merchant_name: Option<String>,
    pub description_or_merchant_name: Option<String>,
    pub original_description: Option<String>,
    pub transaction_type: Option<String>,
    pub location: Option<String>,
    pub check_number: Option<String>,
    pub associated_website: Option<String>,
}

impl TransactionInfo {
    pub fn date(&self) -> NaiveDate {
        // Use authorized date if available (since that's likely the date the user initiated the transaction) and posted date otherwise.
        self.authorized_date.unwrap_or(self.posted_date)
    }
}
