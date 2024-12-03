use anyhow::Result;
use rust_decimal::prelude::Zero as _;
use rust_decimal::Decimal;
use std::collections::{hash_map::Entry, HashMap};
use std::hash::Hash;

use crate::ir::{Ledger, Transaction};

pub fn merge_transactions_with_same_date_and_same_description(ledger: Ledger) -> Ledger {
    let merged_transactions = group_by(
        ledger.transactions.into_iter(),
        |transaction| (transaction.date, transaction.description.clone()),
        |transaction| transaction.postings.into_iter(),
    );

    Ledger {
        transactions: merged_transactions
            .into_iter()
            .map(move |((date, description), postings)| Transaction {
                date,
                description,
                postings,
            })
            .collect(),
    }
}

pub fn check_transactions_are_balanced_per_date(ledger: &Ledger) -> Result<()> {
    let postings_by_date = group_by(
        ledger.transactions.iter(),
        |transaction| transaction.date,
        |transaction| transaction.postings.iter(),
    );
    for (date, postings) in &postings_by_date {
        let sum = postings
            .iter()
            .map(|posting| posting.amount)
            .sum::<Decimal>();
        if sum != Decimal::zero() {
            return Err(anyhow::anyhow!(
                "Postings on date {:?} are not balanced: {:?}",
                date,
                postings,
            ));
        }
    }
    Ok(())
}

fn group_by<T, K, V, IV>(
    items: impl Iterator<Item = T>,
    key_fn: impl Fn(&T) -> K,
    value_fn: impl Fn(T) -> IV,
) -> HashMap<K, Vec<V>>
where
    K: PartialEq + Eq + Hash,
    IV: Iterator<Item = V>,
{
    let mut grouped: HashMap<K, Vec<V>> = HashMap::new();
    for item in items {
        let key = key_fn(&item);
        match grouped.entry(key) {
            Entry::Occupied(mut grouped) => {
                grouped.get_mut().extend(value_fn(item));
            }
            Entry::Vacant(grouped) => {
                grouped.insert(value_fn(item).collect());
            }
        }
    }
    grouped
}
