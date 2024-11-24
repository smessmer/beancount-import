use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DbTransactions {}

impl DbTransactions {
    pub fn new_empty() -> Self {
        Self {}
    }
}
