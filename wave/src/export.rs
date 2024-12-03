use std::{borrow::Cow, io::stdout};

use anyhow::Result;
use beancount_core::{Account, Directive, Flag, IncompleteAmount};
use common_macros::{hash_map, hash_set};

const CURRENCY: &str = "USD";

pub fn print_exported_transactions<'a>(ledger: crate::ir::Ledger) -> Result<()> {
    // TODO Account opening directives with starting balance
    let ledger = beancount_core::Ledger {
        directives: ledger
            .transactions
            .into_iter()
            .map(|transaction| transaction_to_beancount(transaction))
            .collect(),
    };
    if ledger.directives.is_empty() {
        println!("No transactions to export");
    }
    beancount_render::render(&mut stdout(), &ledger)?;
    Ok(())
}

fn transaction_to_beancount(transaction: crate::ir::Transaction) -> Directive<'static> {
    let flag = if transaction.is_balanced() {
        Flag::Okay
    } else {
        Flag::Warning
    };
    Directive::Transaction(beancount_core::Transaction {
        date: transaction.date.into(),
        flag,
        payee: None,
        tags: hash_set![],
        links: hash_set![],
        narration: transaction.description.into(),
        postings: transaction
            .postings
            .into_iter()
            .map(|posting| posting_to_beancount(posting))
            .collect(),
        meta: hash_map![],
        source: None,
    })
}

fn posting_to_beancount(posting: crate::ir::Posting) -> beancount_core::Posting<'static> {
    beancount_core::Posting {
        account: Account {
            // TODO
            ty: beancount_core::AccountType::Assets,
            parts: vec![posting.account_name.into()],
        },
        units: IncompleteAmount {
            num: Some(posting.amount),
            currency: Some(Cow::Borrowed(CURRENCY)),
        },
        cost: None,
        price: None,
        flag: None,
        meta: hash_map![],
    }
}
