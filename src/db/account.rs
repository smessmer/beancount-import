use serde::{Deserialize, Serialize};

use super::{transactions::AddOrVerifyResult, Transaction, TransactionId, Transactions};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountId(pub String);

impl AccountId {
    pub fn new(id: String) -> Self {
        Self(id)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct PlaidAccountInfo {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct BeancountAccountInfo {
    pub ty: AccountType,
    pub name_parts: Vec<String>,
}

impl BeancountAccountInfo {
    pub fn beancount_name(&self) -> String {
        let ty = match self.ty {
            AccountType::Assets => "Assets",
            AccountType::Liabilities => "Liabilities",
            AccountType::Equity => "Equity",
            AccountType::Income => "Income",
            AccountType::Expenses => "Expenses",
        };
        format!("{ty}:{}", self.name_parts.join(":"))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Account {
    pub plaid_account_info: PlaidAccountInfo,
    /// A connection can have multiple accounts, we may have only added some to our database.
    /// Other accounts in the connection will still have an entry but be `None` here.
    pub account: Option<ConnectedAccount>,
}

impl Account {
    pub fn new_connected(
        plaid_account_info: PlaidAccountInfo,
        beancount_account_info: BeancountAccountInfo,
    ) -> Self {
        Self {
            plaid_account_info,
            account: Some(ConnectedAccount {
                beancount_account_info,
                transactions: Transactions::new_empty(),
            }),
        }
    }

    pub fn new_unconnected(plaid_account_info: PlaidAccountInfo) -> Self {
        Self {
            plaid_account_info,
            account: None,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.account.is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct ConnectedAccount {
    pub beancount_account_info: BeancountAccountInfo,
    pub transactions: Transactions,
}

impl ConnectedAccount {
    pub fn add_or_verify_transaction(
        &mut self,
        transaction_id: TransactionId,
        transaction: Transaction,
    ) -> AddOrVerifyResult {
        self.transactions.add_or_verify(transaction_id, transaction)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccountType {
    Assets,
    Liabilities,
    Equity,
    Income,
    Expenses,
}
