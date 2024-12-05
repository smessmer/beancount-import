use chrono::NaiveDate;
use nom::{
    combinator::{all_consuming, cut, eof, opt, value},
    error::{context, VerboseError},
    multi::many_till,
    sequence::{terminated, tuple},
    IResult,
};

mod utils;
use utils::{empty_cell, row_end};

mod account;
mod header;

pub use account::AccountType;

#[derive(Debug, PartialEq, Eq)]
pub struct WaveLedger {
    pub ledger_name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub accounts: Vec<account::Account>,
}

pub fn ledger(input: &str) -> IResult<&str, WaveLedger, VerboseError<&str>> {
    let (input, header) = context("Failed to parse header", header::header)(input)?;
    let (input, (accounts, _eof)) = context(
        "Failed to parse ledger accounts",
        all_consuming(many_till(
            terminated(
                cut(account::account(header.column_schema)),
                opt(row_with_empty_cell),
            ),
            eof,
        )),
    )(input)?;

    Ok((
        input,
        WaveLedger {
            ledger_name: header.ledger_name.to_string(),
            start_date: header.start_date,
            end_date: header.end_date,
            accounts,
        },
    ))
}

fn row_with_empty_cell(input: &str) -> IResult<&str, (), VerboseError<&str>> {
    context(
        "Failed to parse row_with_empty_cell",
        value((), tuple((empty_cell, row_end))),
    )(input)
}

#[cfg(test)]
mod tests {
    use nom::error::{ErrorKind, VerboseErrorKind};
    use rust_decimal::{prelude::Zero, Decimal};

    use super::*;
    use crate::import::parser::account::{Account, EndingBalance, Posting};

    #[test]
    fn test_ledger() {
        let input = r#"Account Transactions
Personal
Date Range: 2024-01-01 to 2024-11-30
Report Type: Accrual (Paid & Unpaid)
ACCOUNT NUMBER,DATE,DESCRIPTION,DEBIT (In Business Currency),CREDIT (In Business Currency),BALANCE (In Business Currency)
,First Account,,,,
Starting Balance,,,,,$123.45
,2024-01-04,Some: Addition,$1.23,,$124.68
,2024-04-04,Some: Withdrawal,,$15.67,$109.01
Totals and Ending Balance,,,$1.23,$15.67,$109.01
Balance Change,,,-$14.44,,
""
,Second Account,,,,
Starting Balance,,,,,$123.45
,2024-01-04,Some: Withdrawal,,$1.23,$122.22
,2024-04-04,Some: Addition,$15.67,,$137.89
Totals and Ending Balance,,,$15.67,$1.23,$137.89
Balance Change,,,$14.44,,"#;
        assert_eq!(
            ledger(input),
            Ok((
                "",
                WaveLedger {
                    ledger_name: "Personal".to_string(),
                    start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                    end_date: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
                    accounts: vec![
                        Account {
                            name: "First Account".to_string(),
                            starting_balance: Decimal::new(12345, 2),
                            postings: vec![
                                Posting {
                                    date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                                    description: "Some: Addition".to_string(),
                                    debit: Decimal::new(123, 2),
                                    credit: Decimal::zero(),
                                    balance: Decimal::new(12468, 2),
                                },
                                Posting {
                                    date: NaiveDate::from_ymd_opt(2024, 4, 4).unwrap(),
                                    description: "Some: Withdrawal".to_string(),
                                    debit: Decimal::zero(),
                                    credit: Decimal::new(1567, 2),
                                    balance: Decimal::new(10901, 2),
                                },
                            ],
                            ending_balance: EndingBalance {
                                total_debit: Decimal::new(123, 2),
                                total_credit: Decimal::new(1567, 2),
                                ending_balance: Decimal::new(10901, 2),
                            },
                            balance_change: Decimal::new(-1444, 2),
                        },
                        Account {
                            name: "Second Account".to_string(),
                            starting_balance: Decimal::new(12345, 2),
                            postings: vec![
                                Posting {
                                    date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                                    description: "Some: Withdrawal".to_string(),
                                    debit: Decimal::zero(),
                                    credit: Decimal::new(123, 2),
                                    balance: Decimal::new(12222, 2),
                                },
                                Posting {
                                    date: NaiveDate::from_ymd_opt(2024, 4, 4).unwrap(),
                                    description: "Some: Addition".to_string(),
                                    debit: Decimal::new(1567, 2),
                                    credit: Decimal::zero(),
                                    balance: Decimal::new(13789, 2),
                                },
                            ],
                            ending_balance: EndingBalance {
                                total_debit: Decimal::new(1567, 2),
                                total_credit: Decimal::new(123, 2),
                                ending_balance: Decimal::new(13789, 2),
                            },
                            balance_change: Decimal::new(1444, 2),
                        }
                    ],
                }
            ))
        );
    }

    #[test]
    fn test_ledger_with_extra_data() {
        let input = r#"Account Transactions
Personal
Date Range: 2024-01-01 to 2024-11-30
Report Type: Accrual (Paid & Unpaid)
ACCOUNT NUMBER,DATE,DESCRIPTION,DEBIT (In Business Currency),CREDIT (In Business Currency),BALANCE (In Business Currency)
,First Account,,,,
Starting Balance,,,,,$123.45
,2024-01-04,Some: Addition,$1.23,,$124.68
,2024-04-04,Some: Withdrawal,,$15.67,$109.01
Totals and Ending Balance,,,$1.23,$15.67,$109.01
Balance Change,,,-$14.44,,
""
,Second Account,,,,
Starting Balance,,,,,$123.45
,2024-01-04,Some: Withdrawal,,$1.23,$122.22
,2024-04-04,Some: Addition,$15.67,,$137.89
Totals and Ending Balance,,,$15.67,$1.23,$137.89
Balance Change,,,$14.44,,
""
bla"#;
        assert_eq!(
            ledger(input),
            Err(nom::Err::Failure(nom::error::VerboseError {
                errors: vec![
                    ("bla", VerboseErrorKind::Nom(ErrorKind::MapRes)),
                    ("bla", VerboseErrorKind::Context("Failed to parse cell_tag")),
                    ("bla", VerboseErrorKind::Context("Failed to parse empty_cell")),
                    ("bla", VerboseErrorKind::Context("Failed to parse account_header_row")),
                    ("bla", VerboseErrorKind::Context("Failed to parse account")),
                    (",First Account,,,,\nStarting Balance,,,,,$123.45\n,2024-01-04,Some: Addition,$1.23,,$124.68\n,2024-04-04,Some: Withdrawal,,$15.67,$109.01\nTotals and Ending Balance,,,$1.23,$15.67,$109.01\nBalance Change,,,-$14.44,,\n\"\"\n,Second Account,,,,\nStarting Balance,,,,,$123.45\n,2024-01-04,Some: Withdrawal,,$1.23,$122.22\n,2024-04-04,Some: Addition,$15.67,,$137.89\nTotals and Ending Balance,,,$15.67,$1.23,$137.89\nBalance Change,,,$14.44,,\n\"\"\nbla", VerboseErrorKind::Context("Failed to parse ledger accounts"))]
            }))
        );
    }

    #[test]
    fn test_row_with_empty_cell() {
        assert_eq!(row_with_empty_cell("\n"), Ok(("", ())));
        assert_eq!(row_with_empty_cell("\r\n"), Ok(("", ())));
        assert_eq!(row_with_empty_cell("\"\"\r\nb"), Ok(("b", ())));
        assert_eq!(
            row_with_empty_cell("foo"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "foo",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "foo",
                        nom::error::VerboseErrorKind::Context("Failed to parse cell_tag")
                    ),
                    (
                        "foo",
                        nom::error::VerboseErrorKind::Context("Failed to parse empty_cell")
                    ),
                    (
                        "foo",
                        nom::error::VerboseErrorKind::Context(
                            "Failed to parse row_with_empty_cell"
                        )
                    )
                ]
            }))
        );
    }
}
