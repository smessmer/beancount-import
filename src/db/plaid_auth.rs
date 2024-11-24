use serde::{Deserialize, Serialize};

const PLAID_VERSION: &str = "2020-09-14";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DbPlaidAuth {
    client_id: String,
    secret: String,
}

impl DbPlaidAuth {
    pub fn new(client_id: String, secret: String) -> Self {
        Self { client_id, secret }
    }

    pub fn to_api_auth(&self) -> plaid::PlaidAuth {
        plaid::PlaidAuth::ClientId {
            client_id: self.client_id.clone(),
            secret: self.secret.clone(),
            plaid_version: PLAID_VERSION.to_string(),
        }
    }
}
