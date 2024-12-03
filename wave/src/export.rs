use std::{borrow::Cow, collections::HashMap, io::stdout};

use anyhow::{anyhow, Result};
use beancount_core::{Amount, Balance, BcOption, Directive, Flag, IncompleteAmount, Open};
use chrono::Days;
use common_macros::{hash_map, hash_set};

use crate::{
    config::Config,
    ir::{self, AccountBalance, Dates, Transaction},
};

const CURRENCY: &str = "USD";

fn opening_balance_account() -> beancount_core::Account<'static> {
    beancount_core::Account {
        ty: beancount_core::AccountType::Equity,
        parts: vec![Cow::Borrowed("Opening-Balances")],
    }
}

pub fn print_exported_transactions<'a>(ledger: crate::ir::Ledger, config: &Config) -> Result<()> {
    print_exported_header(&ledger)?;

    let dates = ledger.dates;
    let balances = ledger.account_balances.clone();
    let mut account_ledgers = group_by_account(ledger, config)?;

    // Don't iterate over account_ledgers because they may not contain all accounts (e.g. they won't contain accounts that have all transactions assigned to other accounts)
    // Instead, iterate over all account names in the ledger. This makes sure we still print account opening directives and balance assertions for accounts that have no transactions.
    for (account, balances) in balances.into_iter() {
        let beancount_account = config.lookup_beancount_account_name(&account)?;
        let transactions = account_ledgers
            .remove(&beancount_account)
            .unwrap_or_else(|| vec![]);

        print_exported_account(
            &account,
            config,
            beancount_account,
            balances,
            dates,
            transactions,
        )?;
    }

    Ok(())
}

fn print_exported_header(ledger: &ir::Ledger) -> Result<()> {
    println!(
        "; Exported from Wave: {ledger_name}\n; Start Date: {start_date}\n; End Date: {end_date}\n",
        ledger_name = ledger.ledger_name,
        start_date = ledger.dates.start_date,
        end_date = ledger.dates.end_date
    );
    let day_before_start_date = ledger
        .dates
        .start_date
        .checked_sub_days(Days::new(1))
        .ok_or_else(|| anyhow!("Failed to subtract a day from the start date"))?;
    let directives = vec![
        Directive::Option(BcOption {
            name: Cow::Borrowed("title"),
            val: Cow::Borrowed(ledger.ledger_name.as_str()),
            source: None,
        }),
        Directive::Option(BcOption {
            name: Cow::Borrowed("operating_currency"),
            val: Cow::Borrowed(CURRENCY),
            source: None,
        }),
        Directive::Open(Open {
            date: day_before_start_date.into(),
            account: opening_balance_account(),
            currencies: vec![Cow::Borrowed(CURRENCY)],
            booking: None,
            meta: hash_map![],
            source: None,
        }),
    ];
    let ledger = beancount_core::Ledger { directives };
    beancount_render::render(&mut stdout(), &ledger)?;

    Ok(())
}

fn print_exported_account(
    import_account_name: &str,
    config: &Config,
    account: beancount_core::Account,
    balances: AccountBalance,
    dates: Dates,
    transactions: Vec<Transaction>,
) -> Result<()> {
    let mut directives = vec![];
    // Open the account a day before the first transaction because the balance assertion must be on the day after the pad directive.
    let day_before_start_date = dates
        .start_date
        .checked_sub_days(Days::new(1))
        .ok_or_else(|| anyhow!("Failed to subtract a day from the start date"))?;
    //  Add the last balance assertion a day after the last transaction because the balance assertion is applied to the beginning of the day.
    let day_after_end_date = dates
        .end_date
        .checked_add_days(Days::new(1))
        .ok_or_else(|| anyhow!("Failed to add a day to the end date"))?;
    directives.push(Directive::Open(Open {
        date: day_before_start_date.into(),
        account: account.clone(),
        currencies: vec![Cow::Borrowed(CURRENCY)],
        booking: None,
        meta: hash_map![],
        source: None,
    }));
    if !balances.start_balance.is_zero() {
        directives.push(Directive::Pad(beancount_core::Pad {
            date: day_before_start_date.into(),
            pad_to_account: account.clone(),
            pad_from_account: opening_balance_account(),
            meta: hash_map![],
            source: None,
        }));
    }
    directives.push(Directive::Balance(Balance {
        date: dates.start_date.into(),
        account: account.clone(),
        amount: Amount {
            num: balances.start_balance,
            currency: Cow::Borrowed(CURRENCY),
        },
        tolerance: None,
        meta: hash_map![],
        source: None,
    }));
    directives.extend(
        transactions
            .into_iter()
            .map(|transaction| transaction_to_beancount(config, transaction))
            .collect::<Result<Vec<_>>>()?
            .into_iter(),
    );
    directives.push(Directive::Balance(Balance {
        date: day_after_end_date.into(),
        account: account.clone(),
        amount: Amount {
            num: balances.end_balance,
            currency: Cow::Borrowed(CURRENCY),
        },
        tolerance: None,
        meta: hash_map![],
        source: None,
    }));
    let ledger = beancount_core::Ledger { directives };

    println!("\n; Imported Account: {import_account_name}\n");
    beancount_render::render(&mut stdout(), &ledger)?;
    println!("\n\n");

    Ok(())
}

fn transaction_to_beancount<'a>(
    config: &'a Config,
    transaction: crate::ir::Transaction,
) -> Result<Directive<'a>> {
    let flag = if transaction.is_balanced() {
        Flag::Okay
    } else {
        Flag::Warning
    };
    Ok(Directive::Transaction(beancount_core::Transaction {
        date: transaction.date.into(),
        flag,
        payee: None,
        tags: hash_set![],
        links: hash_set![],
        narration: transaction.description.into(),
        postings: transaction
            .postings
            .into_iter()
            .map(|posting| posting_to_beancount(config, posting))
            .collect::<Result<Vec<_>>>()?,
        meta: hash_map![],
        source: None,
    }))
}

fn posting_to_beancount<'a>(
    config: &'a Config,
    posting: crate::ir::Posting,
) -> Result<beancount_core::Posting<'a>> {
    Ok(beancount_core::Posting {
        account: config.lookup_beancount_account_name(&posting.account_name)?,
        units: IncompleteAmount {
            num: Some(posting.amount),
            currency: Some(Cow::Borrowed(CURRENCY)),
        },
        cost: None,
        price: None,
        flag: None,
        meta: hash_map![],
    })
}
fn group_by_account(
    ledger: ir::Ledger,
    config: &Config,
) -> Result<HashMap<beancount_core::Account, Vec<Transaction>>> {
    let mut result = HashMap::new();

    for transaction in ledger.transactions {
        let touched_accounts = transaction
            .postings
            .iter()
            .map(|posting| config.lookup_beancount_account_name(&posting.account_name))
            .collect::<Result<Vec<_>>>()?;

        let best_touched_account = touched_accounts
            .into_iter()
            .min_by_key(|account| {
                let account_type_key = match account.ty {
                    beancount_core::AccountType::Assets => 0,
                    beancount_core::AccountType::Liabilities => 1,
                    beancount_core::AccountType::Income => 2,
                    beancount_core::AccountType::Expenses => 3,
                    beancount_core::AccountType::Equity => 4,
                };
                let account_name_key = account
                    .parts
                    .iter()
                    .map(|part| part.clone().into_owned())
                    .reduce(|a, b| format!("{a}:{b}"))
                    .unwrap_or_else(|| "".to_string());
                (account_type_key, account_name_key)
            })
            .ok_or_else(|| anyhow!("No touched accounts"))?;

        result
            .entry(best_touched_account)
            .or_insert_with(Vec::new)
            .push(transaction);
    }

    Ok(result)
}
