use anyhow::Result;
use nom::{error::VerboseError, Finish, Parser};
use std::io::Read;

mod parser;

use parser::WaveLedger;

use crate::ir::{AccountBalance, Dates, Ledger, Posting, Transaction};

pub fn load(input_stream: impl Read) -> Result<Ledger> {
    let wave_ledger = load_wave_ledger(input_stream)?;
    to_ir(wave_ledger)
}

fn load_wave_ledger(mut input_stream: impl Read) -> Result<WaveLedger> {
    let mut content = String::new();
    input_stream.read_to_string(&mut content)?;
    let content = maybe_remove_byte_order_mark(content);
    let (rest, parsed) = parser::ledger
        .parse(&content)
        .finish()
        .map_err(|err| VerboseError {
            errors: err
                .errors
                .into_iter()
                .map(|(input, kind)| (input.to_string(), kind))
                .collect(),
        })?;
    assert_eq!("", rest);
    Ok(parsed)
}

fn maybe_remove_byte_order_mark(mut content: String) -> String {
    if content.starts_with("\u{FEFF}") {
        content.remove(0);
    }
    content
}

fn to_ir(ledger: WaveLedger) -> Result<Ledger> {
    let dates = Dates {
        start_date: ledger.start_date,
        end_date: ledger.end_date,
    };
    let account_balances = ledger
        .accounts
        .iter()
        .map(|account| {
            (
                account.name.clone(),
                AccountBalance {
                    start_balance: account.starting_balance,
                    end_balance: account.ending_balance.ending_balance,
                },
            )
        })
        .collect();
    let transactions = ledger
        .accounts
        .into_iter()
        .flat_map(|account| {
            account.postings.into_iter().map(move |posting| {
                let amount = posting.amount()?;
                Ok::<Transaction, anyhow::Error>(Transaction {
                    date: posting.date,
                    description: posting.description,
                    postings: vec![Posting {
                        account_name: account.name.clone(),
                        amount,
                    }],
                })
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Ledger {
        transactions,
        dates,
        account_balances,
    })
}
