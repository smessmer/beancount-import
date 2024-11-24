use serde::{Deserialize, Serialize};

const PLAID_VERSION: &str = "2020-09-14";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DbPlaidAuth {
    pub client_id: String,
    pub secret: String,
}

impl From<DbPlaidAuth> for plaid::PlaidAuth {
    fn from(auth: DbPlaidAuth) -> Self {
        Self::ClientId {
            client_id: auth.client_id,
            secret: auth.secret,
            plaid_version: PLAID_VERSION.to_string(),
        }
    }
}
