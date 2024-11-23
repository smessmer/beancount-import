use serde::{Deserialize, Serialize};

use super::bank_connection::DbBankConnection;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DatabaseV1 {
    pub bank_connections: Vec<DbBankConnection>,
}

impl DatabaseV1 {
    pub fn new() -> Self {
        Self {
            bank_connections: vec![],
        }
    }
}
