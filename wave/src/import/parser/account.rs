use anyhow::{ensure, Result};
use chrono::NaiveDate;
use nom::{
    combinator::map_res,
    error::{context, VerboseError},
    multi::{count, many0},
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};
use rust_decimal::{prelude::Zero, Decimal};

use super::{
    header::ColumnSchema,
    utils::{amount_cell, amount_cell_opt, cell, cell_tag, comma, date_cell, empty_cell, row_end},
};

#[derive(Debug, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub starting_balance: Decimal,
    pub postings: Vec<Posting>,
    pub ending_balance: EndingBalance,
    pub balance_change: Decimal,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AccountType {
    Debit,
    Credit,
}

impl Account {
    pub fn validate(&self) -> Result<Option<AccountType>, &'static str> {
        let mut account_type = None;
        let mut balance = self.starting_balance;
        let mut total_debit = Decimal::zero();
        let mut total_credit = Decimal::zero();
        for posting in &self.postings {
            if posting.balance == balance + posting.debit - posting.credit {
                match account_type {
                    None => account_type = Some(AccountType::Debit),
                    Some(AccountType::Debit) => {}
                    Some(AccountType::Credit) => return Err("Debit account balance mismatch"),
                }
                balance = posting.balance;
            } else if posting.balance == balance - posting.debit + posting.credit {
                match account_type {
                    None => account_type = Some(AccountType::Credit),
                    Some(AccountType::Debit) => return Err("Credit account balance mismatch"),
                    Some(AccountType::Credit) => {}
                }
                balance = posting.balance;
            } else {
                return Err("Posting balance mismatch");
            }
            total_debit += posting.debit;
            total_credit += posting.credit;
        }
        if total_debit != self.ending_balance.total_debit {
            return Err("Total debit mismatch");
        }
        if total_credit != self.ending_balance.total_credit {
            return Err("Total credit mismatch");
        }
        if balance != self.ending_balance.ending_balance {
            return Err("Ending balance mismatch");
        }
        if self.starting_balance + self.balance_change != self.ending_balance.ending_balance {
            return Err("Balance change mismatch");
        }
        return Ok(account_type);
    }

    pub fn account_type(&self) -> Option<AccountType> {
        // Unwrap is ok because this should already have been validated
        self.validate().unwrap()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Posting {
    pub date: NaiveDate,
    pub description: String,
    pub debit: Decimal,
    pub credit: Decimal,
    pub balance: Decimal,
}

impl Posting {
    pub fn amount(&self) -> Result<Decimal> {
        ensure!(
            self.debit.is_zero() != self.credit.is_zero(),
            "Exactly one of debit and credit must be zero"
        );
        Ok(self.debit - self.credit)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct EndingBalance {
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub ending_balance: Decimal,
}

pub fn account(
    column_schema: ColumnSchema,
) -> impl Fn(&str) -> IResult<&str, Account, VerboseError<&str>> {
    move |input| {
        context("Failed to parse account", |input| {
            let (input, account) = account_unvalidated(column_schema)(input)?;
            if let Err(err) = account.validate() {
                return Err(nom::Err::Failure(VerboseError {
                    errors: vec![(input, nom::error::VerboseErrorKind::Context(err))],
                }));
            }
            Ok((input, account))
        })(input)
    }
}

fn account_unvalidated(
    column_schema: ColumnSchema,
) -> impl Fn(&str) -> IResult<&str, Account, VerboseError<&str>> {
    move |input| {
        let (input, name) = account_header_row(column_schema)(input)?;
        let (input, starting_balance) = starting_balance_row(column_schema)(input)?;
        let (input, postings) = many0(posting_row(column_schema))(input)?;
        let (input, ending_balance) = ending_balance_row(column_schema)(input)?;
        let (input, balance_change) = balance_change_row(column_schema)(input)?;
        let account = Account {
            name,
            starting_balance,
            postings,
            ending_balance,
            balance_change,
        };
        Ok((input, account))
    }
}

fn account_header_row(
    column_schema: ColumnSchema,
) -> impl Fn(&str) -> IResult<&str, String, VerboseError<&str>> {
    let num_commas_at_end = match column_schema {
        ColumnSchema::GlobalLedgerCurrency => 4,
        ColumnSchema::PerAccountCurrency => 10,
    };
    move |input| {
        context(
            "Failed to parse account_header_row",
            delimited(
                tuple((empty_cell, comma)),
                cell,
                tuple((
                    count(tuple((comma, empty_cell)), num_commas_at_end),
                    row_end,
                )),
            ),
        )(input)
    }
}

fn starting_balance_row(
    column_schema: ColumnSchema,
) -> impl Fn(&str) -> IResult<&str, Decimal, VerboseError<&str>> {
    move |input| {
        let amount_in_ledger_currency = preceded(
            tuple((
                cell_tag("Starting Balance"),
                count(tuple((comma, empty_cell)), 4),
                comma,
            )),
            amount_cell,
        );
        match column_schema {
            ColumnSchema::GlobalLedgerCurrency => context(
                "Failed to parse starting_balance_row",
                map_res(terminated(amount_in_ledger_currency, row_end), |amount| {
                    ensure!(amount.currency_symbol == '$', "Currency symbol is not $");
                    Ok(amount.amount)
                }),
            )(input),
            ColumnSchema::PerAccountCurrency => context(
                "Failed to parse starting_balance_row",
                map_res(
                    tuple((
                        amount_in_ledger_currency,
                        comma,
                        cell,
                        count(tuple((comma, empty_cell)), 3),
                        comma,
                        amount_cell,
                        comma,
                        cell,
                        row_end,
                    )),
                    |(
                        amount_in_ledger_currency,
                        (),
                        ledger_currency,
                        _,
                        (),
                        amount_in_account_currency,
                        (),
                        account_currency,
                        (),
                    )| {
                        ensure!(ledger_currency == "USD", "Ledger currency is not USD");
                        // TODO Handle non-USD account currencies
                        ensure!(account_currency == "USD", "Account currency is not USD");
                        ensure!(
                            amount_in_ledger_currency == amount_in_account_currency,
                            "Amounts in ledger and account currency do not match"
                        );
                        ensure!(
                            amount_in_ledger_currency.currency_symbol == '$',
                            "Currency symbol is not $"
                        );
                        Ok(amount_in_ledger_currency.amount)
                    },
                ),
            )(input),
        }
    }
}

fn posting_row(
    column_schema: ColumnSchema,
) -> impl Fn(&str) -> IResult<&str, Posting, VerboseError<&str>> {
    move |input| {
        let common_columns = map_res(
            tuple((
                empty_cell,
                comma,
                date_cell,
                comma,
                cell,
                comma,
                amount_cell_opt,
                comma,
                amount_cell_opt,
                comma,
                amount_cell,
            )),
            |((), (), date, (), description, (), debit, (), credit, (), balance)| {
                let debit = match debit {
                    Some(debit) => {
                        // TODO Handle non-USD currencies
                        ensure!(debit.currency_symbol == '$', "Currency symbol is not $");
                        debit.amount
                    }
                    None => Decimal::zero(),
                };
                let credit = match credit {
                    Some(credit) => {
                        // TODO Handle non-USD currencies
                        ensure!(credit.currency_symbol == '$', "Currency symbol is not $");
                        credit.amount
                    }
                    None => Decimal::zero(),
                };
                ensure!(balance.currency_symbol == '$', "Currency symbol is not $");
                let balance = balance.amount;
                Ok(Posting {
                    date,
                    description,
                    debit,
                    credit,
                    balance,
                })
            },
        );
        match column_schema {
            ColumnSchema::GlobalLedgerCurrency => context(
                "Failed to parse posting_row",
                terminated(common_columns, row_end),
            )(input),
            ColumnSchema::PerAccountCurrency => context(
                "Failed to parse posting_row",
                map_res(
                    tuple((
                        common_columns,
                        comma,
                        cell,
                        comma,
                        empty_cell,
                        comma,
                        amount_cell_opt,
                        comma,
                        amount_cell_opt,
                        comma,
                        amount_cell,
                        comma,
                        cell,
                        row_end,
                    )),
                    |(
                        posting,
                        (),
                        ledger_currency,
                        (),
                        (),
                        (),
                        debit_in_account_currency,
                        (),
                        credit_in_account_currency,
                        (),
                        balance_in_account_currency,
                        (),
                        account_currency,
                        (),
                    )| {
                        ensure!(ledger_currency == "USD", "Ledger currency is not USD");
                        // TODO Handle non-USD account currencies
                        ensure!(account_currency == "USD", "Account currency is not USD");
                        let debit_in_account_currency = match debit_in_account_currency {
                            Some(debit) => {
                                ensure!(debit.currency_symbol == '$', "Currency symbol is not $");
                                Some(debit.amount)
                            }
                            None => None,
                        };
                        let credit_in_account_currency = match credit_in_account_currency {
                            Some(credit) => {
                                ensure!(credit.currency_symbol == '$', "Currency symbol is not $");
                                Some(credit.amount)
                            }
                            None => None,
                        };
                        ensure!(
                            balance_in_account_currency.currency_symbol == '$',
                            "Currency symbol is not $"
                        );
                        let balance_in_account_currency = balance_in_account_currency.amount;
                        ensure!(
                            debit_in_account_currency.is_some()
                                || credit_in_account_currency.is_some(),
                            "Either debit or credit must be present"
                        );
                        ensure!(
                            debit_in_account_currency.unwrap_or(Decimal::zero()) == posting.debit,
                            "Debit in account currency does not match debit in ledger currency"
                        );
                        ensure!(
                            credit_in_account_currency.unwrap_or(Decimal::zero()) == posting.credit,
                            "Credit in account currency does not match credit in ledger currency"
                        );
                        ensure!(
                            balance_in_account_currency == posting.balance,
                            "Balance in account currency does not match balance in ledger currency"
                        );
                        Ok(posting)
                    },
                ),
            )(input),
        }
    }
}

fn ending_balance_row(
    column_schema: ColumnSchema,
) -> impl Fn(&str) -> IResult<&str, EndingBalance, VerboseError<&str>> {
    move |input| {
        let common_columns = map_res(
            tuple((
                cell_tag("Totals and Ending Balance"),
                comma,
                empty_cell,
                comma,
                empty_cell,
                comma,
                amount_cell,
                comma,
                amount_cell,
                comma,
                amount_cell,
            )),
            |(_, (), (), (), (), (), total_debit, (), total_credit, (), ending_balance)| {
                ensure!(
                    total_debit.currency_symbol == '$',
                    "Currency symbol is not $"
                );
                ensure!(
                    total_credit.currency_symbol == '$',
                    "Currency symbol is not $"
                );
                ensure!(
                    ending_balance.currency_symbol == '$',
                    "Currency symbol is not $"
                );
                Ok(EndingBalance {
                    total_debit: total_debit.amount,
                    total_credit: total_credit.amount,
                    ending_balance: ending_balance.amount,
                })
            },
        );
        match column_schema {
            ColumnSchema::GlobalLedgerCurrency => context(
                "Failed to parse ending_balance_row",
                terminated(common_columns, row_end),
            )(input),
            ColumnSchema::PerAccountCurrency => {
                context(
                    "Failed to parse ending_balance_row",
                    map_res(
                        tuple((
                            common_columns,
                            comma,
                            cell,
                            comma,
                            empty_cell,
                            comma,
                            amount_cell,
                            comma,
                            amount_cell,
                            comma,
                            amount_cell,
                            comma,
                            cell,
                            row_end,
                        )),
                        |(
                            ending_balance,
                            (),
                            ledger_currency,
                            (),
                            (),
                            (),
                            total_debit_in_account_currency,
                            (),
                            total_credit_in_account_currency,
                            (),
                            ending_balance_in_account_currency,
                            (),
                            account_currency,
                            (),
                        )| {
                            ensure!(ledger_currency == "USD", "Ledger currency is not USD");
                            // TODO Handle non-USD account currencies
                            ensure!(account_currency == "USD", "Account currency is not USD");
                            ensure!(
                                total_debit_in_account_currency.currency_symbol == '$',
                                "Currency symbol is not $"
                            );
                            ensure!(
                                total_credit_in_account_currency.currency_symbol == '$',
                                "Currency symbol is not $"
                            );
                            ensure!(
                                ending_balance_in_account_currency.currency_symbol == '$',
                                "Currency symbol is not $"
                            );
                            ensure!(
                                total_debit_in_account_currency.amount == ending_balance.total_debit,
                                "Total debit in account currency does not match total debit in ledger currency"
                            );
                            ensure!(
                                total_credit_in_account_currency.amount == ending_balance.total_credit,
                                "Total credit in account currency does not match total credit in ledger currency"
                            );
                            ensure!(
                                ending_balance_in_account_currency.amount == ending_balance.ending_balance,
                                "Ending balance in account currency does not match ending balance in ledger currency"
                            );
                            Ok(ending_balance)
                        },
                    ),
                )(input)
            }
        }
    }
}

fn balance_change_row(
    column_schema: ColumnSchema,
) -> impl Fn(&str) -> IResult<&str, Decimal, VerboseError<&str>> {
    move |input| {
        let common_rows = delimited(
            tuple((
                cell_tag("Balance Change"),
                comma,
                empty_cell,
                comma,
                empty_cell,
                comma,
            )),
            amount_cell,
            tuple((comma, empty_cell, comma, empty_cell)),
        );
        match column_schema {
            ColumnSchema::GlobalLedgerCurrency => context(
                "Failed to parse balance_change_row",
                map_res(terminated(common_rows, row_end), |amount| {
                    ensure!(amount.currency_symbol == '$', "Currency symbol is not $");
                    Ok(amount.amount)
                }),
            )(input),
            ColumnSchema::PerAccountCurrency => context(
                "Failed to parse balance_change_row",
                map_res(
                    tuple((
                        common_rows,
                        comma,
                        cell,
                        comma,
                        empty_cell,
                        comma,
                        amount_cell,
                        comma,
                        empty_cell,
                        comma,
                        empty_cell,
                        comma,
                        cell,
                        row_end,
                    )),
                    |(
                        balance_change,
                        (),
                        ledger_currency,
                        (),
                        (),
                        (),
                        balance_change_in_account_currency,
                        (),
                        (),
                        (),
                        (),
                        (),
                        account_currency,
                        (),
                    )| {
                        ensure!(ledger_currency == "USD", "Ledger currency is not USD");
                        // TODO Handle non-USD account currencies
                        ensure!(account_currency == "USD", "Account currency is not USD");
                        ensure!(
                            balance_change_in_account_currency == balance_change,
                            "Balance change in account currency does not match balance change in ledger currency"
                        );
                        ensure!(
                            balance_change.currency_symbol == '$',
                            "Currency symbol is not $"
                        );
                        ensure!(
                            balance_change_in_account_currency.currency_symbol == '$',
                            "Currency symbol is not $"
                        );
                        Ok(balance_change.amount)
                    },
                ),
            )(input),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_global_schema_test_account_header_row() {
        let input = ",My Bank Account,,,,\nbla";
        assert_eq!(
            account_header_row(ColumnSchema::GlobalLedgerCurrency)(input),
            Ok(("bla", "My Bank Account".to_string()))
        );
    }

    #[test]
    fn given_peraccount_schema_test_account_header_row() {
        let input = ",My Bank Account,,,,,,,,,,\nbla";
        assert_eq!(
            account_header_row(ColumnSchema::PerAccountCurrency)(input),
            Ok(("bla", "My Bank Account".to_string()))
        );
    }

    #[test]
    fn given_global_schema_test_starting_balance_row() {
        let input = "Starting Balance,,,,,\"$12,345.67\"\nbla";
        assert_eq!(
            starting_balance_row(ColumnSchema::GlobalLedgerCurrency)(input),
            Ok(("bla", Decimal::new(1234567, 2)))
        );
    }

    #[test]
    fn given_peraccount_schema_test_starting_balance_row() {
        let input = "Starting Balance,,,,,\"$12,345.67\",USD,,,,\"$12,345.67\",USD\nbla";
        assert_eq!(
            starting_balance_row(ColumnSchema::PerAccountCurrency)(input),
            Ok(("bla", Decimal::new(1234567, 2)))
        );
    }

    #[test]
    fn given_global_schema_test_posting_row_credit() {
        let input = ",2024-01-04,Some description,,$123.45,\"$1,234.56\"\nbla";
        assert_eq!(
            posting_row(ColumnSchema::GlobalLedgerCurrency)(input),
            Ok((
                "bla",
                Posting {
                    date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                    description: "Some description".to_string(),
                    debit: Decimal::new(0, 0),
                    credit: Decimal::new(12345, 2),
                    balance: Decimal::new(123456, 2),
                }
            ))
        );
    }

    #[test]
    fn given_peraccount_schema_test_posting_row_credit() {
        let input = ",2024-01-04,Some description,,$123.45,\"$1,234.56\",USD,,,$123.45,\"$1,234.56\",USD\nbla";
        assert_eq!(
            posting_row(ColumnSchema::PerAccountCurrency)(input),
            Ok((
                "bla",
                Posting {
                    date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                    description: "Some description".to_string(),
                    debit: Decimal::new(0, 0),
                    credit: Decimal::new(12345, 2),
                    balance: Decimal::new(123456, 2),
                }
            ))
        );
    }

    #[test]
    fn given_global_schema_test_posting_row_debit() {
        let input = ",2024-02-01,Some description,\"$1,234.56\",,\"$2,345.67\"\nbla";
        assert_eq!(
            posting_row(ColumnSchema::GlobalLedgerCurrency)(input),
            Ok((
                "bla",
                Posting {
                    date: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
                    description: "Some description".to_string(),
                    debit: Decimal::new(123456, 2),
                    credit: Decimal::new(0, 0),
                    balance: Decimal::new(234567, 2),
                }
            ))
        );
    }

    #[test]
    fn given_peraccount_schema_test_posting_row_debit() {
        let input = ",2024-02-01,Some description,\"$1,234.56\",,\"$2,345.67\",USD,,\"$1,234.56\",,\"$2,345.67\",USD\nbla";
        assert_eq!(
            posting_row(ColumnSchema::PerAccountCurrency)(input),
            Ok((
                "bla",
                Posting {
                    date: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
                    description: "Some description".to_string(),
                    debit: Decimal::new(123456, 2),
                    credit: Decimal::new(0, 0),
                    balance: Decimal::new(234567, 2),
                }
            ))
        );
    }

    #[test]
    fn given_global_schema_test_ending_balance_row() {
        let input =
            "Totals and Ending Balance,,,\"$123,456.78\",\"$234,567.89\",\"$45,678.90\"\nbla";
        assert_eq!(
            ending_balance_row(ColumnSchema::GlobalLedgerCurrency)(input),
            Ok((
                "bla",
                EndingBalance {
                    total_debit: Decimal::new(12345678, 2),
                    total_credit: Decimal::new(23456789, 2),
                    ending_balance: Decimal::new(4567890, 2),
                }
            ))
        );
    }

    #[test]
    fn given_peraccount_schema_test_ending_balance_row() {
        let input =
            "Totals and Ending Balance,,,\"$123,456.78\",\"$234,567.89\",\"$45,678.90\",USD,,\"$123,456.78\",\"$234,567.89\",\"$45,678.90\",USD\nbla";
        assert_eq!(
            ending_balance_row(ColumnSchema::PerAccountCurrency)(input),
            Ok((
                "bla",
                EndingBalance {
                    total_debit: Decimal::new(12345678, 2),
                    total_credit: Decimal::new(23456789, 2),
                    ending_balance: Decimal::new(4567890, 2),
                }
            ))
        );
    }

    #[test]
    fn given_global_schema_test_balance_change_row() {
        let input = "Balance Change,,,\"$9,876.54\",,\nbla";
        assert_eq!(
            balance_change_row(ColumnSchema::GlobalLedgerCurrency)(input),
            Ok(("bla", Decimal::new(987654, 2)))
        );
    }

    #[test]
    fn given_peraccount_schema_test_balance_change_row() {
        let input = "Balance Change,,,\"$9,876.54\",,,USD,,\"$9,876.54\",,,USD\nbla";
        assert_eq!(
            balance_change_row(ColumnSchema::PerAccountCurrency)(input),
            Ok(("bla", Decimal::new(987654, 2)))
        );
    }

    // TODO Add peraccount_schema variants of the following tests

    #[test]
    fn test_account_empty() {
        let input = r#",My Bank Account,,,,
Starting Balance,,,,,$12.34
Totals and Ending Balance,,,$0.00,$0.00,"$12.34"
Balance Change,,,"$0.0",,"#;
        assert_eq!(
            // TODO Also test with PerAccountCurrency
            account(ColumnSchema::GlobalLedgerCurrency)(input),
            Ok((
                "",
                Account {
                    name: "My Bank Account".to_string(),
                    starting_balance: Decimal::new(1234, 2),
                    postings: vec![],
                    ending_balance: EndingBalance {
                        total_debit: Decimal::zero(),
                        total_credit: Decimal::zero(),
                        ending_balance: Decimal::new(1234, 2),
                    },
                    balance_change: Decimal::zero(),
                }
            ))
        );
    }

    #[test]
    fn test_account_valid_with_negative_change() {
        let input = r#",Some Account,,,,
Starting Balance,,,,,$123.45
,2024-01-04,Some: Addition,$1.23,,$124.68
,2024-04-04,Some: Withdrawal,,$15.67,$109.01
Totals and Ending Balance,,,$1.23,$15.67,$109.01
Balance Change,,,-$14.44,,"#;
        assert_eq!(
            // TODO Also test with PerAccountCurrency
            account(ColumnSchema::GlobalLedgerCurrency)(input),
            Ok((
                "",
                Account {
                    name: "Some Account".to_string(),
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
                }
            ))
        );
    }

    #[test]
    fn test_account_valid_with_positive_change() {
        let input = r#",Some Account,,,,
Starting Balance,,,,,$123.45
,2024-01-04,Some: Withdrawal,,$1.23,$122.22
,2024-04-04,Some: Addition,$15.67,,$137.89
Totals and Ending Balance,,,$15.67,$1.23,$137.89
Balance Change,,,$14.44,,"#;
        assert_eq!(
            // TODO Also test with PerAccountCurrency
            account(ColumnSchema::GlobalLedgerCurrency)(input),
            Ok((
                "",
                Account {
                    name: "Some Account".to_string(),
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
            ))
        );
    }
}
