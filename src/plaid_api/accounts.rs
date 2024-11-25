use anyhow::Result;

use crate::db::{AccessToken, AccountId, PlaidAccountInfo};

use super::client::Plaid;

pub async fn get_accounts(
    client: &Plaid,
    access_token: &AccessToken,
) -> Result<impl Iterator<Item = (AccountId, PlaidAccountInfo)>> {
    log::info!("Requesting accounts...");

    let response = client.client().accounts_get(access_token.get()).await?;
    let result = response.accounts.into_iter().map(|account| {
        (
            AccountId(account.account_id),
            PlaidAccountInfo { name: account.name },
        )
    });

    log::info!("Requesting accounts...done");
    Ok(result)
}
