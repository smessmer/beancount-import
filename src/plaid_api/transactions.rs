use anyhow::{anyhow, ensure, Result};
use plaid::model::TransactionsSyncRequestOptions;
use rust_decimal::{prelude::FromPrimitive as _, Decimal};

use super::client::Plaid;
use crate::db::{AccessToken, AccountId, Amount, TransactionCategory, TransactionId};

pub async fn get_transactions(
    client: &Plaid,
    access_token: &AccessToken,
) -> Result<Vec<TransactionWithAccount>> {
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

#[derive(Debug)]
pub struct TransactionWithAccount {
    pub account_id: AccountId,
    pub transaction: crate::db::Transaction,
}

struct TransactionsPage {
    transactions: Vec<TransactionWithAccount>,
    next_page_cursor: Option<String>,
}

async fn sync_transactions_page(
    client: &Plaid,
    access_token: &AccessToken,
    cursor: Option<String>,
) -> Result<TransactionsPage> {
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
    let transactions = response
        .added
        .into_iter()
        .flat_map(|transaction| {
            if transaction.transaction_base.pending {
                log::warn!("Ignoring pending transaction: {:?}", transaction);
                None
            } else {
                let amount = match Decimal::from_f64(transaction.transaction_base.amount) {
                    Some(amount) => amount,
                    None => {
                        return Some(Err(anyhow!(
                            "Failed to parse amount {}",
                            transaction.transaction_base.amount
                        )))
                    }
                };
                let date = transaction.authorized_date.unwrap_or(transaction.date);
                Some(Ok(TransactionWithAccount {
                    account_id: AccountId::new(transaction.transaction_base.account_id),
                    transaction: crate::db::Transaction {
                        id: TransactionId(transaction.transaction_base.transaction_id),
                        merchant_name: transaction.transaction_base.merchant_name,
                        description: transaction.transaction_base.original_description,
                        date,
                        category: transaction.personal_finance_category.map(|category| {
                            TransactionCategory {
                                primary: category.primary,
                                detailed: category.detailed,
                            }
                        }),
                        amount: Amount {
                            amount,
                            iso_currency_code: transaction.transaction_base.iso_currency_code,
                        },
                    },
                }))
            }
        })
        .collect::<Result<_>>()?;
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
