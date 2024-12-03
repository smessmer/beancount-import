use std::{borrow::Cow, io::stdout};

use anyhow::Result;
use beancount_core::{Directive, Flag, IncompleteAmount};
use common_macros::{hash_map, hash_set};

use crate::config::Config;

const CURRENCY: &str = "USD";

pub fn print_exported_transactions<'a>(ledger: crate::ir::Ledger, config: &Config) -> Result<()> {
    // TODO Account opening directives with starting balance
    let ledger = beancount_core::Ledger {
        directives: ledger
            .transactions
            .into_iter()
            .map(|transaction| transaction_to_beancount(transaction, &config))
            .collect::<Result<Vec<_>>>()?,
    };
    if ledger.directives.is_empty() {
        println!("No transactions to export");
    }
    beancount_render::render(&mut stdout(), &ledger)?;
    Ok(())
}

fn transaction_to_beancount<'a>(
    transaction: crate::ir::Transaction,
    config: &'a Config,
) -> Result<Directive<'a>> {
    let flag = if transaction.is_balanced() {
        Flag::Okay
    } else {
        Flag::Warning
    };
    Ok(Directive::Transaction(beancount_core::Transaction {
        date: transaction.date.into(),
        flag,
        payee: None,
        tags: hash_set![],
        links: hash_set![],
        narration: transaction.description.into(),
        postings: transaction
            .postings
            .into_iter()
            .map(|posting| posting_to_beancount(posting, config))
            .collect::<Result<Vec<_>>>()?,
        meta: hash_map![],
        source: None,
    }))
}

fn posting_to_beancount<'a>(
    posting: crate::ir::Posting,
    config: &'a Config,
) -> Result<beancount_core::Posting<'a>> {
    let account = config.lookup_beancount_account_name(&posting.account_name)?;
    Ok(beancount_core::Posting {
        account,
        units: IncompleteAmount {
            num: Some(posting.amount),
            currency: Some(Cow::Borrowed(CURRENCY)),
        },
        cost: None,
        price: None,
        flag: None,
        meta: hash_map![],
    })
}
