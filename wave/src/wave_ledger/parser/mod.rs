use chrono::NaiveDate;
use nom::{
    combinator::{all_consuming, cut, opt, value},
    error::{context, VerboseError},
    multi::many0,
    sequence::{terminated, tuple},
    IResult,
};

mod utils;
use utils::{empty_cell, row_end};

mod account;
mod header;

#[derive(Debug, PartialEq, Eq)]
pub struct Ledger {
    start_date: NaiveDate,
    end_date: NaiveDate,
    accounts: Vec<account::Account>,
}

pub fn ledger(input: &str) -> IResult<&str, Ledger, VerboseError<&str>> {
    let (input, (start_date, end_date)) = context("Failed to parse header", header::header)(input)?;
    let (input, accounts) = context(
        "Failed to parse ledger accounts",
        all_consuming(many0(terminated(
            cut(account::account),
            opt(row_with_empty_cell),
        ))),
    )(input)?;

    Ok((
        input,
        Ledger {
            start_date,
            end_date,
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
    use rust_decimal::{prelude::Zero, Decimal};

    use super::*;
    use crate::wave_ledger::parser::account::{Account, EndingBalance, Posting};

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
                Ledger {
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
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![(
                    "bla",
                    nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::Eof)
                )]
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
