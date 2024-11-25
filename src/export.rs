use std::{borrow::Cow, io::stdout};

use anyhow::Result;
use beancount_core::{metadata::MetaValue, Directive, Flag, IncompleteAmount, Ledger, Posting};
use common_macros::{hash_map, hash_set};

use crate::db::{AccountType, BeancountAccountInfo, ConnectedAccount, Transaction, TransactionId};

pub fn export_transactions<'a>(
    transactions: impl Iterator<Item = (&'a ConnectedAccount, &'a TransactionId, &'a Transaction)>,
) -> Result<()> {
    let ledger = Ledger {
        directives: transactions
            .map(|(account, id, t)| {
                transaction_to_beancount(&account.beancount_account_info, id, t)
            })
            .collect(),
    };
    beancount_render::render(&mut stdout(), &ledger)?;
    Ok(())
}

fn transaction_to_beancount<'a>(
    account: &'a BeancountAccountInfo,
    transaction_id: &'a TransactionId,
    transaction: &'a Transaction,
) -> Directive<'a> {
    let mut meta = hash_map![
        Cow::Borrowed("plaid_transaction_id") => MetaValue::Text(Cow::Borrowed(&transaction_id.0)),
    ];
    if let Some(category) = &transaction.category {
        meta.insert(
            Cow::Borrowed("plaid_category"),
            MetaValue::Text(Cow::Owned(format!(
                "{}.{}",
                category.primary, category.detailed,
            ))),
        );
    }
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
            account: account_to_beancount(account),
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
            meta,
        }],
        meta: hash_map![],
        source: None,
    })
}

fn account_to_beancount<'a>(account: &'a BeancountAccountInfo) -> beancount_core::Account<'a> {
    let ty = match account.ty {
        AccountType::Assets => beancount_core::AccountType::Assets,
        AccountType::Liabilities => beancount_core::AccountType::Liabilities,
        AccountType::Equity => beancount_core::AccountType::Equity,
        AccountType::Income => beancount_core::AccountType::Income,
        AccountType::Expenses => beancount_core::AccountType::Expenses,
    };
    let parts = account
        .name_parts
        .iter()
        .map(|v| Cow::Borrowed(v.as_str()))
        .collect();
    beancount_core::Account { ty, parts }
}
