use std::{borrow::Cow, io::stdout};

use anyhow::Result;
use beancount_core::{AccountType, Directive, Flag, IncompleteAmount, Ledger, Posting};
use common_macros::{hash_map, hash_set};

use crate::db::Transaction;

pub fn export_transactions<'a>(transactions: impl Iterator<Item = &'a Transaction>) -> Result<()> {
    let ledger = Ledger {
        directives: transactions.map(transaction_to_directive).collect(),
    };
    beancount_render::render(&mut stdout(), &ledger)?;
    Ok(())
}

fn transaction_to_directive(transaction: &Transaction) -> Directive {
    // TODO Should we export the transaction id somehow? maybe as a label, tag or meta?
    Directive::Transaction(beancount_core::Transaction {
        date: transaction.date.into(),
        flag: Flag::Warning, // TODO What flag?
        payee: None,         // TODO
        narration: transaction
            .description
            .as_deref()
            .map(Cow::Borrowed)
            .unwrap_or(Cow::Borrowed("")),
        tags: hash_set![],  // TODO
        links: hash_set![], // TODO
        postings: vec![Posting {
            account: beancount_core::Account {
                ty: AccountType::Assets,                                     // TODO
                parts: vec![Cow::Borrowed("Part1"), Cow::Borrowed("Part2")], // TODO
            },
            units: IncompleteAmount {
                num: Some(transaction.amount.amount),
                currency: transaction
                    .amount
                    .iso_currency_code
                    .as_deref()
                    .map(Cow::Borrowed),
            },
            cost: None,        // TODO
            price: None,       // TODO
            flag: None,        // TODO
            meta: hash_map![], // TODO
        }],
        meta: hash_map![], // TODO
        source: None,      // TODO
    })
}
