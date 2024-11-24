use anyhow::Result;
use plaid::{model::LinkTokenCreateRequestUser, request::LinkTokenCreateRequired};

use crate::plaid_api::{AccessToken, Plaid};

use super::{
    link_http_server,
    tokens::{LinkToken, PublicToken},
};

const CLIENT_NAME: &str = "beancount-plaid";
const COUNTRY_CODES: &[&str] = &["US"];
const LANGUAGE: &str = "en";
const USER_ID: &str = "user-id";
const PRODUCTS: &[&str] = &["transactions"];

/// Link a new account and return the access token. This will launch an in-browser account linking flow with Plaid's UI
pub async fn link_new_account(client: &Plaid) -> Result<AccessToken> {
    log::info!("Requesting link token...");
    let link_token: LinkToken = link_token_create(client).await?;
    log::info!("Requesting link token...done");

    log::info!("Initiating link flow...");
    let public_token = link_http_server::link_in_browser(link_token).await?;
    log::info!("Initiating link flow...done");

    log::info!("Requesting access token...");
    let access_token = exchange_public_token(client, public_token).await?;
    log::info!("Requesting access token...done");
    Ok(access_token)
}

pub async fn link_token_create(client: &Plaid) -> Result<LinkToken> {
    let response = client
        .client()
        .link_token_create(LinkTokenCreateRequired {
            client_name: CLIENT_NAME,
            country_codes: COUNTRY_CODES,
            language: LANGUAGE,
            user: LinkTokenCreateRequestUser {
                client_user_id: USER_ID.to_string(),
                ..Default::default()
            },
        })
        .products(PRODUCTS)
        .await?;
    Ok(LinkToken(response.link_token))
}

async fn exchange_public_token(client: &Plaid, public_token: PublicToken) -> Result<AccessToken> {
    let response = client
        .client()
        .item_public_token_exchange(&public_token.0)
        .await?;
    Ok(AccessToken::new(response.access_token))
}
