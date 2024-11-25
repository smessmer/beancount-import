use std::{borrow::Cow, io::stdout};

use anyhow::Result;
use beancount_core::{
    metadata::MetaValue, AccountType, Directive, Flag, IncompleteAmount, Ledger, Posting,
};
use common_macros::{hash_map, hash_set};

use crate::db::{Transaction, TransactionId};

pub fn export_transactions<'a>(
    transactions: impl Iterator<Item = (&'a TransactionId, &'a Transaction)>,
) -> Result<()> {
    let ledger = Ledger {
        directives: transactions
            .map(|(id, t)| transaction_to_directive(id, t))
            .collect(),
    };
    beancount_render::render(&mut stdout(), &ledger)?;
    Ok(())
}

fn transaction_to_directive<'a>(
    transaction_id: &'a TransactionId,
    transaction: &'a Transaction,
) -> Directive<'a> {
    Directive::Transaction(beancount_core::Transaction {
        date: transaction.date.into(),
        flag: Flag::Warning,
        payee: transaction.merchant_name.as_deref().map(Cow::Borrowed),
        narration: transaction
            .description
            .as_deref()
            .map(Cow::Borrowed)
            .unwrap_or(Cow::Borrowed("")),
        tags: hash_set![],
        links: hash_set![],
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
            cost: None,
            price: None,
            flag: None,
            meta: hash_map![
                Cow::Borrowed("plaid_transaction_id") => MetaValue::Text(Cow::Borrowed(&transaction_id.0)),
            ],
        }],
        meta: hash_map![],
        source: None,
    })
}
