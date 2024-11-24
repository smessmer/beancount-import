use serde::{Deserialize, Serialize};

use super::{bank_connection::BankConnection, plaid_auth::DbPlaidAuth};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DatabaseV1 {
    pub plaid_auth: DbPlaidAuth,
    pub bank_connections: Vec<BankConnection>,
}

impl DatabaseV1 {
    pub fn new(plaid_auth: DbPlaidAuth) -> Self {
        Self {
            plaid_auth,
            bank_connections: vec![],
        }
    }
}
