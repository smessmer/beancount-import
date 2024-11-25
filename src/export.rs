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
    let date = if let Some(authorized_date) = transaction.authorized_date {
        // Transaction has both a posted and an authorized date. Let's report the authorized date
        // as the transaction date, but add metadata with the posted date.
        meta.insert(
            Cow::Borrowed("posted_date"),
            MetaValue::Date(transaction.posted_date.into()),
        );
        authorized_date
    } else {
        transaction.posted_date
    };
    if let Some(location) = &transaction.location {
        if location != "{}" {
            meta.insert(
                Cow::Borrowed("plaid_location"),
                MetaValue::Text(Cow::Owned(location.clone())),
            );
        }
    }
    if let Some(website) = &transaction.associated_website {
        meta.insert(
            Cow::Borrowed("plaid_associated_website"),
            MetaValue::Text(Cow::Borrowed(website)),
        );
    }
    if let Some(check_number) = &transaction.check_number {
        meta.insert(
            Cow::Borrowed("plaid_check_number"),
            MetaValue::Text(Cow::Borrowed(check_number)),
        );
    }
    Directive::Transaction(beancount_core::Transaction {
        date: date.into(),
        flag: Flag::Warning,
        payee: transaction.merchant_name.as_deref().map(Cow::Borrowed),
        narration: transaction
            .description_or_merchant_name
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
