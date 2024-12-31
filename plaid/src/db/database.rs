use serde::{Deserialize, Serialize};

use super::{bank_connection::BankConnection, plaid_auth::DbPlaidAuth};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DatabaseV1 {
    pub plaid_auth: DbPlaidAuth,
    pub bank_connections: Vec<BankConnection>,
}

/// Format changes since DatabaseV1:
/// * all transaction amounts are multplied by `-1`
#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct DatabaseV2 {
    pub plaid_auth: DbPlaidAuth,
    pub bank_connections: Vec<BankConnection>,
}

impl DatabaseV2 {
    pub fn new(plaid_auth: DbPlaidAuth) -> Self {
        Self {
            plaid_auth,
            bank_connections: vec![],
        }
    }

    pub fn migrate(database: DatabaseV1) -> Self {
        let DatabaseV1 {
            plaid_auth,
            bank_connections,
        } = database;

        let bank_connections = bank_connections
            .into_iter()
            .map(|mut connection| {
                for (_, account) in connection.accounts_mut() {
                    if let Some(connected_account) = &mut account.account {
                        for (_id, transaction) in
                            connected_account.transactions.iter_all_sorted_by_date_mut()
                        {
                            transaction.transaction.amount.amount =
                                -transaction.transaction.amount.amount;
                        }
                    }
                }
                connection
            })
            .collect();

        Self {
            plaid_auth,
            bank_connections,
        }
    }
}
