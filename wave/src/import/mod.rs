use anyhow::Result;
use ariadne::{Color, Fmt as _, Label, Report, ReportKind, Source};
use nom::{error::VerboseError, Finish as _, Parser as _};
use rust_decimal::{prelude::Zero, Decimal};
use std::io::Read;

mod parser;

use parser::{AccountType, WaveLedger};

use crate::ir::{AccountBalance, Dates, Ledger, Posting, Transaction};

pub fn load(input_stream: impl Read) -> Result<Ledger> {
    let wave_ledger = load_wave_ledger(input_stream)?;
    to_ir(wave_ledger)
}

fn load_wave_ledger(mut input_stream: impl Read) -> Result<WaveLedger> {
    let mut content = String::new();
    input_stream.read_to_string(&mut content)?;
    let content = maybe_remove_byte_order_mark(content);
    // match cell_tag_chumsky("Account Transactions")
    //     .then(row_end_chumsky())
    //     .then(cell_tag_chumsky("Personal"))
    //     .then(row_end_chumsky())
    //     .then(cell_tag_chumsky("Blub"))
    //     .parse(content.as_str())
    // {
    //     Ok(_) => (),
    //     Err(errors) => {
    //         for err in errors {
    //             print_parser_error(&content, err)
    //         }
    //     }
    // }
    // todo!();
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

fn print_parser_error(input: &str, err: chumsky::error::Simple<char>) {
    // Taken from https://github.com/zesterer/chumsky/blob/0.9/examples/json.rs
    let msg = if let chumsky::error::SimpleReason::Custom(msg) = err.reason() {
        msg.clone()
    } else {
        format!(
            "{}{}, expected {}",
            if err.found().is_some() {
                "Unexpected token"
            } else {
                "Unexpected end of input"
            },
            if let Some(label) = err.label() {
                format!(" while parsing {}", label)
            } else {
                String::new()
            },
            if err.expected().len() == 0 {
                "something else".to_string()
            } else {
                err.expected()
                    .map(|expected| match expected {
                        Some(expected) => expected.to_string(),
                        None => "end of input".to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        )
    };

    let report = Report::build(ReportKind::Error, err.span())
        .with_message(msg)
        .with_label(
            Label::new(err.span())
                .with_message(match err.reason() {
                    chumsky::error::SimpleReason::Custom(msg) => msg.clone(),
                    _ => format!(
                        "Unexpected {}",
                        err.found()
                            .map(|c| format!("token {}", c.fg(Color::Red)))
                            .unwrap_or_else(|| "end of input".to_string())
                    ),
                })
                .with_color(Color::Red),
        );

    let report = match err.reason() {
        chumsky::error::SimpleReason::Unclosed { span, delimiter } => report.with_label(
            Label::new(span.clone())
                .with_message(format!(
                    "Unclosed delimiter {}",
                    delimiter.fg(Color::Yellow)
                ))
                .with_color(Color::Yellow),
        ),
        chumsky::error::SimpleReason::Unexpected => report,
        chumsky::error::SimpleReason::Custom(_) => report,
    };

    report.finish().print(Source::from(&input)).unwrap();
}

fn maybe_remove_byte_order_mark(mut content: String) -> String {
    if content.starts_with("\u{FEFF}") {
        content.remove(0);
    }
    content
}

fn to_ir(ledger: WaveLedger) -> Result<Ledger> {
    let ledger_name = ledger.ledger_name;
    let dates = Dates {
        start_date: ledger.start_date,
        end_date: ledger.end_date,
    };
    let account_balances = ledger
        .accounts
        .iter()
        .map(|account| {
            Ok((
                account.name.clone(),
                match account.account_type() {
                    Some(AccountType::Debit) => AccountBalance {
                        start_balance: account.starting_balance,
                        end_balance: account.ending_balance.ending_balance,
                    },
                    Some(AccountType::Credit) => AccountBalance {
                        start_balance: -account.starting_balance,
                        end_balance: -account.ending_balance.ending_balance,
                    },
                    None => {
                        if account.starting_balance.is_zero() && account.ending_balance.ending_balance.is_zero() {
                            AccountBalance {
                                start_balance: Decimal::zero(),
                                end_balance: Decimal::zero(),
                            }
                        } else {
                            anyhow::bail!(
                                "Couldn't determine account type (debit vs credit) of account '{}'. ",
                                account.name
                            );
                        }
                    }
                },
            ))
        })
        .collect::<Result<_>>()?;
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
        ledger_name,
        transactions,
        dates,
        account_balances,
    })
}
