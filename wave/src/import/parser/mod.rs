use chrono::NaiveDate;
use chumsky::{error::Simple, prelude::end, Parser as _};

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

pub fn ledger() -> impl chumsky::Parser<char, WaveLedger, Error = Simple<char>> {
    header::header().then_with(|header| {
        account::account(header.column_schema)
            .separated_by(row_with_empty_cell())
            .then_ignore(row_with_empty_cell().or_not())
            .then_ignore(end())
            .map(move |accounts| WaveLedger {
                ledger_name: header.ledger_name.to_string(),
                start_date: header.start_date,
                end_date: header.end_date,
                accounts,
            })
    })
}

fn row_with_empty_cell() -> impl chumsky::Parser<char, (), Error = Simple<char>> {
    empty_cell()
        .then_ignore(row_end())
        .labelled("row with empty cell")
}

#[cfg(test)]
mod tests {
    use chumsky::Error;
    use rust_decimal::{prelude::Zero, Decimal};
    use utils::test_parser;

    use super::*;
    use crate::{
        import::parser::account::{Account, EndingBalance, Posting},
        ir::{Amount, LEDGER_CURRENCY},
    };

    // TODO Add tests for ledgers with per-account currencies

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
        test_parser(
            input,
            ledger(),
            WaveLedger {
                ledger_name: "Personal".to_string(),
                start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
                accounts: vec![
                    Account {
                        name: "First Account".to_string(),
                        account_currency: LEDGER_CURRENCY.to_string(),
                        starting_balance: Amount {
                            in_account_currency: Decimal::new(12345, 2),
                            in_ledger_currency: Decimal::new(12345, 2),
                        },
                        postings: vec![
                            Posting {
                                date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                                description: "Some: Addition".to_string(),
                                debit: Amount {
                                    in_ledger_currency: Decimal::new(123, 2),
                                    in_account_currency: Decimal::new(123, 2),
                                },
                                credit: Amount {
                                    in_ledger_currency: Decimal::zero(),
                                    in_account_currency: Decimal::zero(),
                                },
                                balance: Amount {
                                    in_ledger_currency: Decimal::new(12468, 2),
                                    in_account_currency: Decimal::new(12468, 2),
                                },
                            },
                            Posting {
                                date: NaiveDate::from_ymd_opt(2024, 4, 4).unwrap(),
                                description: "Some: Withdrawal".to_string(),
                                debit: Amount {
                                    in_ledger_currency: Decimal::zero(),
                                    in_account_currency: Decimal::zero(),
                                },
                                credit: Amount {
                                    in_ledger_currency: Decimal::new(1567, 2),
                                    in_account_currency: Decimal::new(1567, 2),
                                },
                                balance: Amount {
                                    in_ledger_currency: Decimal::new(10901, 2),
                                    in_account_currency: Decimal::new(10901, 2),
                                },
                            },
                        ],
                        ending_balance: EndingBalance {
                            total_debit: Amount {
                                in_ledger_currency: Decimal::new(123, 2),
                                in_account_currency: Decimal::new(123, 2),
                            },
                            total_credit: Amount {
                                in_ledger_currency: Decimal::new(1567, 2),
                                in_account_currency: Decimal::new(1567, 2),
                            },
                            ending_balance: Amount {
                                in_ledger_currency: Decimal::new(10901, 2),
                                in_account_currency: Decimal::new(10901, 2),
                            },
                        },
                        balance_change: Amount {
                            in_ledger_currency: Decimal::new(-1444, 2),
                            in_account_currency: Decimal::new(-1444, 2),
                        },
                    },
                    Account {
                        name: "Second Account".to_string(),
                        account_currency: LEDGER_CURRENCY.to_string(),
                        starting_balance: Amount {
                            in_ledger_currency: Decimal::new(12345, 2),
                            in_account_currency: Decimal::new(12345, 2),
                        },
                        postings: vec![
                            Posting {
                                date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                                description: "Some: Withdrawal".to_string(),
                                debit: Amount {
                                    in_ledger_currency: Decimal::zero(),
                                    in_account_currency: Decimal::zero(),
                                },
                                credit: Amount {
                                    in_ledger_currency: Decimal::new(123, 2),
                                    in_account_currency: Decimal::new(123, 2),
                                },
                                balance: Amount {
                                    in_ledger_currency: Decimal::new(12222, 2),
                                    in_account_currency: Decimal::new(12222, 2),
                                },
                            },
                            Posting {
                                date: NaiveDate::from_ymd_opt(2024, 4, 4).unwrap(),
                                description: "Some: Addition".to_string(),
                                debit: Amount {
                                    in_ledger_currency: Decimal::new(1567, 2),
                                    in_account_currency: Decimal::new(1567, 2),
                                },
                                credit: Amount {
                                    in_ledger_currency: Decimal::zero(),
                                    in_account_currency: Decimal::zero(),
                                },
                                balance: Amount {
                                    in_ledger_currency: Decimal::new(13789, 2),
                                    in_account_currency: Decimal::new(13789, 2),
                                },
                            },
                        ],
                        ending_balance: EndingBalance {
                            total_debit: Amount {
                                in_ledger_currency: Decimal::new(1567, 2),
                                in_account_currency: Decimal::new(1567, 2),
                            },
                            total_credit: Amount {
                                in_ledger_currency: Decimal::new(123, 2),
                                in_account_currency: Decimal::new(123, 2),
                            },
                            ending_balance: Amount {
                                in_ledger_currency: Decimal::new(13789, 2),
                                in_account_currency: Decimal::new(13789, 2),
                            },
                        },
                        balance_change: Amount {
                            in_ledger_currency: Decimal::new(1444, 2),
                            in_account_currency: Decimal::new(1444, 2),
                        },
                    },
                ],
            },
            "",
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
            ledger().parse(input),
            Err(vec![
                Simple::expected_input_found(654..655, [None], Some('b')).with_label("csv cell"),
                Simple::custom(654..657, "Failed to parse cell content").with_label("csv cell")
            ])
        );
    }

    #[test]
    fn test_row_with_empty_cell() {
        test_parser("\n", row_with_empty_cell(), (), "");
        test_parser("\r\n", row_with_empty_cell(), (), "");
        test_parser("\"\"\r\nb", row_with_empty_cell(), (), "b");
        assert_eq!(
            row_with_empty_cell().parse("foo"),
            Err(vec![
                Simple::expected_input_found(0..1, [None], Some('f')).with_label("csv cell"),
                Simple::custom(0..3, "Failed to parse cell content").with_label("csv cell")
            ])
        );
    }
}
