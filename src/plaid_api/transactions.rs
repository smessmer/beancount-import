use anyhow::{ensure, Result};
use chrono::NaiveDate;
use plaid::model::TransactionsSyncRequestOptions;
use std::fmt::Debug;

use super::{accounts::AccountId, client::Plaid, AccessToken};

pub struct Amount {
    amount: f64,
    iso_currency_code: Option<String>,
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

pub struct TransactionCategory {
    primary: String,
    detailed: String,
}

impl Debug for TransactionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.primary, self.detailed,)
    }
}

#[derive(Debug)]
pub struct Transaction {
    account: AccountId,
    merchant_name: Option<String>,
    description: Option<String>,
    date: Option<NaiveDate>,
    category: Option<TransactionCategory>,
    amount: Amount,
    pending: bool,
}

pub async fn get_transactions(
    client: &Plaid,
    access_token: &AccessToken,
) -> Result<Vec<Transaction>> {
    log::info!("Requesting transactions...");
    log::info!("Requesting transactions...page 1...");

    let mut result = Vec::new();

    let mut page = sync_transactions_page(client, access_token, None).await?;
    result.extend(page.transactions);

    let mut pagenum = 1;
    while let Some(next_page_cursor) = page.next_page_cursor {
        pagenum += 1;
        log::info!("Requesting transactions...page {pagenum}...");
        page = sync_transactions_page(client, access_token, Some(next_page_cursor)).await?;
        result.extend(page.transactions);
    }

    log::info!("Requesting transactions...done");

    Ok(result)
}

struct TransactionsPage<I>
where
    I: Iterator<Item = Transaction> + ExactSizeIterator,
{
    transactions: I,
    next_page_cursor: Option<String>,
}

async fn sync_transactions_page(
    client: &Plaid,
    access_token: &AccessToken,
    cursor: Option<String>,
) -> Result<TransactionsPage<impl Iterator<Item = Transaction> + ExactSizeIterator>> {
    let mut request = client
        .client()
        .transactions_sync(access_token.get())
        .options(TransactionsSyncRequestOptions {
            include_original_description: Some(true), // TODO Are we actually using this?
            ..Default::default()
        });
    if let Some(cursor) = cursor {
        request = request.cursor(&cursor);
    }
    let response = request.await?;

    ensure!(response.modified.is_empty(), "Got modified transactions but expected only added transactions, we're not doing delta sync.");
    ensure!(response.removed.is_empty(), "Got removed transactions but expected only added transactions, we're not doing delta sync.");
    let transactions = response.added.into_iter().map(|transaction| Transaction {
        account: AccountId::new(transaction.transaction_base.account_id),
        merchant_name: transaction.transaction_base.merchant_name,
        description: transaction.transaction_base.original_description,
        date: transaction.authorized_date,
        category: transaction
            .personal_finance_category
            .map(|category| TransactionCategory {
                primary: category.primary,
                detailed: category.detailed,
            }),
        amount: Amount {
            amount: transaction.transaction_base.amount,
            iso_currency_code: transaction.transaction_base.iso_currency_code,
        },
        pending: transaction.transaction_base.pending,
    });
    let next_page_cursor = if response.has_more {
        Some(response.next_cursor)
    } else {
        None
    };
    Ok(TransactionsPage {
        transactions,
        next_page_cursor,
    })
}
