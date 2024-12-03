use anyhow::{anyhow, Context, Result};
use beancount_core::AccountType;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::HashMap};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub beancount_account_names: HashMap<String, AccountConfig>,
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        for (name, account) in &self.beancount_account_names {
            account
                .beancount_name()
                .with_context(|| anyhow!("Error in account {}: {}", name, account.0))?;
        }
        Ok(())
    }

    pub fn lookup_beancount_account_name(&self, name: &str) -> Result<beancount_core::Account> {
        self.beancount_account_names
            .get(name)
            .with_context(|| anyhow!("Account not found: {}", name))?
            .beancount_name()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountConfig(String);

impl AccountConfig {
    pub fn beancount_name(&self) -> Result<beancount_core::Account> {
        // TODO Deduplicate with parse_beancount_account_name function in //plaid/src/db/account.rs
        let mut parts = self.0.split(':');
        let ty = parts
            .next()
            .expect("There should always be at least one part to the split");
        let ty = match ty {
            "Assets" => AccountType::Assets,
            "Liabilities" => AccountType::Liabilities,
            "Equity" => AccountType::Equity,
            "Income" => AccountType::Income,
            "Expenses" => AccountType::Expenses,
            _ => {
                return Err(anyhow!(
            "Account must start with one of: Assets:, Liabilities:, Equity:, Income:, Expenses:",
        ))
            }
        };
        Ok(beancount_core::Account {
            ty,
            parts: parts.map(Cow::Borrowed).collect(),
        })
    }
}

pub fn prompt_edit_config(imported_account_names: impl Iterator<Item = String>) -> Result<Config> {
    let initial_config = Config {
        beancount_account_names: imported_account_names
            .map(|name| (name.clone(), AccountConfig("".to_string())))
            .collect(),
    };
    let serialized = serde_yaml::to_string(&initial_config)?;
    let Some(edited) = dialoguer::Editor::new().edit(&serialized)? else {
        return Err(anyhow!("You did not save the edits, please try again"));
    };
    let new_config: Config = serde_yaml::from_str(&edited)?;
    new_config.validate()?;

    Ok(new_config)
}
