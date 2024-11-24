use anyhow::Result;

use crate::db::{AccessToken, AccountId, AccountInfo};

use super::client::Plaid;

pub async fn get_accounts(client: &Plaid, access_token: &AccessToken) -> Result<Vec<AccountInfo>> {
    log::info!("Requesting accounts...");

    let response = client.client().accounts_get(access_token.get()).await?;
    let result = response
        .accounts
        .into_iter()
        .map(|account| AccountInfo {
            id: AccountId(account.account_id),
            name: account.name,
        })
        .collect();

    log::info!("Requesting accounts...done");
    Ok(result)
}
