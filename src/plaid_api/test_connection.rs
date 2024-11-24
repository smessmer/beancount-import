use anyhow::Result;

use super::{link_account::link_token_create, Plaid};

pub async fn test_connection(client: &Plaid) -> Result<()> {
    // The easiest way to test the connection is to create a link token
    link_token_create(client).await?;
    Ok(())
}
