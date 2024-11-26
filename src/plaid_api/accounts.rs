use anyhow::{anyhow, Result};

use crate::db::{AccessToken, AccountId, PlaidAccountInfo};

use super::client::Plaid;

pub async fn get_accounts(
    client: &Plaid,
    access_token: &AccessToken,
) -> Result<impl Iterator<Item = Result<(AccountId, PlaidAccountInfo)>> + ExactSizeIterator> {
    log::info!("Requesting accounts...");

    let response = client.client().accounts_get(access_token.get()).await?;
    let result = response.accounts.into_iter().map(|account| {
        Ok((
            AccountId(account.account_id),
            PlaidAccountInfo {
                name: account.name,
                official_name: account.official_name,
                mask: account.mask,
                type_: account.type_,
                subtype: account
                    .subtype
                    .map(|subtype| match subtype.0 {
                        serde_json::Value::String(s) => Ok(s),
                        _ => Err(anyhow!(
                            "Expected string for account subtype but got {:?}",
                            subtype
                        )),
                    })
                    .transpose()?,
            },
        ))
    });

    log::info!("Requesting accounts...done");
    Ok(result)
}
