use anyhow::Result;
use chrono::NaiveDate;
use rust_decimal::prelude::Zero as _;
use rust_decimal::Decimal;
use std::collections::{hash_map::Entry, HashMap};
use std::hash::Hash;

use crate::ir::{Ledger, Posting, Transaction};

pub fn merge_transactions_with_same_date_description_and_amount(ledger: Ledger) -> Ledger {
    let merged_transactions = group_by(
        ledger.transactions.into_iter(),
        |transaction| (transaction.date, transaction.description.clone()),
        |transaction| transaction.postings.into_iter(),
    );

    Ledger {
        ledger_name: ledger.ledger_name,
        dates: ledger.dates,
        account_balances: ledger.account_balances,
        transactions: merged_transactions
            .into_iter()
            .flat_map(move |((date, description), postings)| {
                transactions_from_postings(date, description, postings)
            })
            .collect(),
    }
}

// Take all postings from a given date with a given description and generate transactions.
// Any two postings with matching amounts will be merged to one transaction.
// But if there is ambiguity, i.e. there are more than two postings with the same amount, they will be left as individual transactions.
// Other postings will become individual transactions.
fn transactions_from_postings(
    date: NaiveDate,
    description: String,
    postings: Vec<Posting>,
) -> impl Iterator<Item = Transaction> {
    let mut postings_by_amount: HashMap<Decimal, Vec<Posting>> = HashMap::new();
    for posting in postings {
        match postings_by_amount.entry(posting.amount.in_ledger_currency) {
            Entry::Occupied(mut postings) => {
                postings.get_mut().push(posting);
            }
            Entry::Vacant(postings) => {
                postings.insert(vec![posting]);
            }
        }
    }

    let mut result = vec![];

    while let Some(amount) = postings_by_amount.keys().into_iter().copied().next() {
        let positive_postings = postings_by_amount.remove(&amount).unwrap();
        let negative_postings = postings_by_amount.remove(&-amount).unwrap_or_default();

        if positive_postings.len() == 1 && negative_postings.len() == 1 {
            let positive_posting = positive_postings.into_iter().next().unwrap();
            let negative_posting = negative_postings.into_iter().next().unwrap();
            result.push(Transaction {
                date,
                description: description.clone(),
                postings: vec![positive_posting, negative_posting],
            });
        } else {
            result.extend(
                positive_postings
                    .into_iter()
                    .chain(negative_postings.into_iter())
                    .map(|posting| Transaction {
                        date,
                        description: description.clone(),
                        postings: vec![posting],
                    }),
            );
        }
    }

    result.into_iter()
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
            .map(|posting| posting.amount.in_ledger_currency)
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

pub fn sort_transactions_by_date(mut ledger: Ledger) -> Ledger {
    ledger
        .transactions
        .sort_by_key(|transaction| transaction.date);
    ledger
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
