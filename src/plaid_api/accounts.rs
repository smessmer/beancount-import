use anyhow::Result;

use super::{client::Plaid, AccessToken};

#[derive(Debug)]
pub struct AccountId(String);

#[derive(Debug)]
pub struct AccountInfo {
    id: AccountId,
    name: String,
}

pub async fn get_accounts(client: &Plaid, access_token: &AccessToken) -> Result<Vec<AccountInfo>> {
    let response = client.client().accounts_get(access_token.get()).await?;
    Ok(response
        .accounts
        .into_iter()
        .map(|account| AccountInfo {
            id: AccountId(account.account_id),
            name: account.name,
        })
        .collect())
}
