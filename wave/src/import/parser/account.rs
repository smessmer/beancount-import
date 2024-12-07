use anyhow::{ensure, Result};
use chrono::NaiveDate;
use chumsky::{error::Simple, Parser as _};
use rust_decimal::{prelude::Zero, Decimal};

use super::{
    header::ColumnSchema,
    utils::{
        amount_cell, amount_cell_opt, any_cell, cell_tag, comma, date_cell, empty_cell, row_end,
    },
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
) -> impl chumsky::Parser<char, Account, Error = Simple<char>> {
    account_header_row(column_schema)
        .then(starting_balance_row(column_schema))
        .then(posting_row(column_schema).repeated())
        .then(ending_balance_row(column_schema))
        .then(balance_change_row(column_schema))
        .try_map(
            |((((name, starting_balance), postings), ending_balance), balance_change), span| {
                let account = Account {
                    name,
                    starting_balance,
                    postings,
                    ending_balance,
                    balance_change,
                };
                account
                    .validate()
                    .map_err(|err| Simple::custom(span, err))?;
                Ok(account)
            },
        )
        .labelled("account")
}

fn account_header_row(
    column_schema: ColumnSchema,
) -> impl chumsky::Parser<char, String, Error = Simple<char>> {
    let num_commas_at_end = match column_schema {
        ColumnSchema::GlobalLedgerCurrency => 4,
        ColumnSchema::PerAccountCurrency => 10,
    };
    empty_cell()
        .then(comma())
        .ignore_then(any_cell())
        .then_ignore(
            comma()
                .ignore_then(empty_cell())
                .repeated()
                .exactly(num_commas_at_end)
                .ignore_then(row_end()),
        )
        .labelled("account header row")
}

fn starting_balance_row(
    column_schema: ColumnSchema,
) -> impl chumsky::Parser<char, Decimal, Error = Simple<char>> {
    let amount_in_ledger_currency = cell_tag("Starting Balance")
        .ignore_then(comma().ignore_then(empty_cell()).repeated().exactly(4))
        .ignore_then(comma())
        .ignore_then(amount_cell());
    let parser = match column_schema {
        ColumnSchema::GlobalLedgerCurrency => amount_in_ledger_currency
            .then_ignore(row_end())
            .try_map(|amount, span| {
                if amount.currency_symbol != '$' {
                    return Err(Simple::custom(span, "Currency symbol is not $"));
                }
                Ok(amount.amount)
            })
            .boxed(),
        ColumnSchema::PerAccountCurrency => amount_in_ledger_currency
            .then_ignore(comma())
            .then(any_cell())
            .then_ignore(comma().ignore_then(empty_cell()).repeated().exactly(3))
            .then_ignore(comma())
            .then(amount_cell())
            .then_ignore(comma())
            .then(any_cell())
            .then_ignore(row_end())
            .try_map(
                |(
                    ((amount_in_ledger_currency, ledger_currency), amount_in_account_currency),
                    account_currency,
                ),
                 span| {
                    if ledger_currency != "USD" {
                        return Err(Simple::custom(span, "Ledger currency is not USD"));
                    }
                    // TODO Handle non-USD account currencies
                    if account_currency != "USD" {
                        return Err(Simple::custom(span, "Account currency is not USD"));
                    }
                    if amount_in_ledger_currency != amount_in_account_currency {
                        return Err(Simple::custom(
                            span,
                            "Amounts in ledger and account currency do not match",
                        ));
                    }
                    if amount_in_ledger_currency.currency_symbol != '$' {
                        return Err(Simple::custom(span, "Currency symbol is not $"));
                    }
                    Ok(amount_in_ledger_currency.amount)
                },
            )
            .boxed(),
    };
    parser.labelled("starting balance row")
}

fn posting_row(
    column_schema: ColumnSchema,
) -> impl chumsky::Parser<char, Posting, Error = Simple<char>> {
    let common_columns = empty_cell()
        .ignore_then(comma())
        .ignore_then(date_cell())
        .then_ignore(comma())
        .then(any_cell())
        .then_ignore(comma())
        .then(amount_cell_opt())
        .then_ignore(comma())
        .then(amount_cell_opt())
        .then_ignore(comma())
        .then(amount_cell())
        .try_map(|((((date, description), debit), credit), balance), span| {
            let debit = match debit {
                Some(debit) => {
                    // TODO Handle non-USD currencies
                    if debit.currency_symbol != '$' {
                        return Err(Simple::custom(span, "Currency symbol is not $"));
                    }
                    debit.amount
                }
                None => Decimal::zero(),
            };
            let credit = match credit {
                Some(credit) => {
                    // TODO Handle non-USD currencies
                    if credit.currency_symbol != '$' {
                        return Err(Simple::custom(span, "Currency symbol is not $"));
                    }
                    credit.amount
                }
                None => Decimal::zero(),
            };
            if balance.currency_symbol != '$' {
                return Err(Simple::custom(span, "Currency symbol is not $"));
            }
            let balance = balance.amount;
            Ok(Posting {
                date,
                description,
                debit,
                credit,
                balance,
            })
        });
    let parser = match column_schema {
        ColumnSchema::GlobalLedgerCurrency => common_columns.then_ignore(row_end()).boxed(),
        ColumnSchema::PerAccountCurrency => common_columns
            .then_ignore(comma())
            .then(any_cell())
            .then_ignore(comma())
            .then_ignore(empty_cell())
            .then_ignore(comma())
            .then(amount_cell_opt())
            .then_ignore(comma())
            .then(amount_cell_opt())
            .then_ignore(comma())
            .then(amount_cell())
            .then_ignore(comma())
            .then(any_cell())
            .then_ignore(row_end())
            .try_map(
                |(
                    (
                        (
                            ((posting, ledger_currency), debit_in_account_currency),
                            credit_in_account_currency,
                        ),
                        balance_in_account_currency,
                    ),
                    account_currency,
                ),
                 span| {
                    if ledger_currency != "USD" {
                        return Err(Simple::custom(span, "Ledger currency is not USD"));
                    }
                    // TODO Handle non-USD account currencies
                    if account_currency != "USD" {
                        return Err(Simple::custom(span, "Account currency is not USD"));
                    }
                    let debit_in_account_currency = match debit_in_account_currency {
                        Some(debit) => {
                            if debit.currency_symbol != '$' {
                                return Err(Simple::custom(span, "Currency symbol is not $"));
                            }
                            Some(debit.amount)
                        }
                        None => None,
                    };
                    let credit_in_account_currency = match credit_in_account_currency {
                        Some(credit) => {
                            if credit.currency_symbol != '$' {
                                return Err(Simple::custom(span, "Currency symbol is not $"));
                            }
                            Some(credit.amount)
                        }
                        None => None,
                    };
                    if balance_in_account_currency.currency_symbol != '$' {
                        return Err(Simple::custom(span, "Currency symbol is not $"));
                    }
                    let balance_in_account_currency = balance_in_account_currency.amount;
                    if debit_in_account_currency.is_some() == credit_in_account_currency.is_some() {
                        return Err(Simple::custom(
                            span,
                            "Exactly one of debit and credit must be present",
                        ));
                    }
                    if debit_in_account_currency.unwrap_or(Decimal::zero()) != posting.debit {
                        return Err(Simple::custom(
                            span,
                            "Debit in account currency does not match debit in ledger currency",
                        ));
                    }
                    if credit_in_account_currency.unwrap_or(Decimal::zero()) != posting.credit {
                        return Err(Simple::custom(
                            span,
                            "Credit in account currency does not match credit in ledger currency",
                        ));
                    }
                    if balance_in_account_currency != posting.balance {
                        return Err(Simple::custom(
                            span,
                            "Balance in account currency does not match balance in ledger currency",
                        ));
                    }
                    Ok(posting)
                },
            )
            .boxed(),
    };
    parser.labelled("posting row")
}

fn ending_balance_row(
    column_schema: ColumnSchema,
) -> impl chumsky::Parser<char, EndingBalance, Error = Simple<char>> {
    let common_columns = cell_tag("Totals and Ending Balance")
        .then_ignore(comma())
        .then_ignore(empty_cell())
        .then_ignore(comma())
        .then_ignore(empty_cell())
        .then_ignore(comma())
        .ignore_then(amount_cell())
        .then_ignore(comma())
        .then(amount_cell())
        .then_ignore(comma())
        .then(amount_cell())
        .try_map(|((total_debit, total_credit), ending_balance), span| {
            if total_debit.currency_symbol != '$' {
                return Err(Simple::custom(span, "Currency symbol is not $"));
            }
            if total_credit.currency_symbol != '$' {
                return Err(Simple::custom(span, "Currency symbol is not $"));
            }
            if ending_balance.currency_symbol != '$' {
                return Err(Simple::custom(span, "Currency symbol is not $"));
            }
            Ok(EndingBalance {
                total_debit: total_debit.amount,
                total_credit: total_credit.amount,
                ending_balance: ending_balance.amount,
            })
        });
    let parser = match column_schema {
        ColumnSchema::GlobalLedgerCurrency =>
            common_columns.then_ignore(row_end()).boxed(),
        ColumnSchema::PerAccountCurrency =>
            common_columns
                .then_ignore(comma())
                .then(any_cell())
                .then_ignore(comma())
                .then_ignore(empty_cell())
                .then_ignore(comma())
                .then(amount_cell())
                .then_ignore(comma())
                .then(amount_cell())
                .then_ignore(comma())
                .then(amount_cell())
                .then_ignore(comma())
                .then(any_cell())
                .then_ignore(row_end())
                .try_map(
                | (
                    ((((ending_balance,
                    ledger_currency),
                    total_debit_in_account_currency),
                    total_credit_in_account_currency),
                    ending_balance_in_account_currency),
                    account_currency,
                ), span
                | {
                    if ledger_currency != "USD" {
                        return Err(Simple::custom(span, "Ledger currency is not USD"));
                    }
                    // TODO Handle non-USD account currencies
                    if account_currency != "USD" {
                        return Err(Simple::custom(span, "Account currency is not USD"));
                    }
                    if total_debit_in_account_currency.currency_symbol != '$' {
                        return Err(Simple::custom(span, "Currency symbol is not $"));
                    }
                    if total_credit_in_account_currency.currency_symbol != '$' {
                        return Err(Simple::custom(span, "Currency symbol is not $"));
                    }
                    if ending_balance_in_account_currency.currency_symbol != '$' {
                        return Err(Simple::custom(span, "Currency symbol is not $"));
                    }
                    if total_debit_in_account_currency.amount != ending_balance.total_debit {
                        return Err(Simple::custom(
                            span,
                            "Total debit in account currency does not match total debit in ledger currency",
                        ));
                    }
                    if total_credit_in_account_currency.amount != ending_balance.total_credit {
                        return Err(Simple::custom(
                            span,
                            "Total credit in account currency does not match total credit in ledger currency",
                        ));
                    }
                    if ending_balance_in_account_currency.amount != ending_balance.ending_balance {
                        return Err(Simple::custom(
                            span,
                            "Ending balance in account currency does not match ending balance in ledger currency",
                        ));
                    }
                    Ok(ending_balance)
                },
        ).boxed()
    };
    parser.labelled("ending balance row")
}

fn balance_change_row(
    column_schema: ColumnSchema,
) -> impl chumsky::Parser<char, Decimal, Error = Simple<char>> {
    let common_rows = cell_tag("Balance Change")
        .then_ignore(comma())
        .then_ignore(empty_cell())
        .then_ignore(comma())
        .then_ignore(empty_cell())
        .then_ignore(comma())
        .ignore_then(amount_cell())
        .then_ignore(comma())
        .then_ignore(empty_cell())
        .then_ignore(comma())
        .then_ignore(empty_cell());
    let parser = match column_schema {
        ColumnSchema::GlobalLedgerCurrency =>
            common_rows.then_ignore(row_end()).try_map(|amount, span| {
                if amount.currency_symbol != '$' {
                    return Err(Simple::custom(span, "Currency symbol is not $"));
                }
                Ok(amount.amount)
            }).boxed(),
        ColumnSchema::PerAccountCurrency =>
            common_rows
                .then_ignore(comma())
                .then(any_cell())
                .then_ignore(comma())
                .then_ignore(empty_cell())
                .then_ignore(comma())
                .then(amount_cell())
                .then_ignore(comma())
                .then_ignore(empty_cell())
                .then_ignore(comma())
                .then_ignore(empty_cell())
                .then_ignore(comma())
                .then(any_cell())
                .then_ignore(row_end())
                .try_map(|(
                    ((balance_change, ledger_currency), balance_change_in_account_currency),
                    account_currency,
                ), span| {
                    if ledger_currency != "USD" {
                        return Err(Simple::custom(span, "Ledger currency is not USD"));
                    }
                    // TODO Handle non-USD account currencies
                    if account_currency != "USD" {
                        return Err(Simple::custom(span, "Account currency is not USD"));
                    }
                    if balance_change != balance_change_in_account_currency {
                        return Err(Simple::custom(
                            span,
                            "Balance change in ledger currency does not match balance change in account currency",
                        ));
                    }
                    if balance_change.currency_symbol != '$' {
                        return Err(Simple::custom(span, "Currency symbol is not $"));
                    }
                    if balance_change_in_account_currency.currency_symbol != '$' {
                        return Err(Simple::custom(span, "Currency symbol is not $"));
                    }
                    Ok(balance_change.amount)
                }).boxed()
        };
    parser.labelled("balance change row")
}

#[cfg(test)]
mod tests {
    use crate::import::parser::utils::test_parser;

    use super::*;

    #[test]
    fn given_global_schema_test_account_header_row() {
        let input = ",My Bank Account,,,,\nbla";
        test_parser(
            input,
            account_header_row(ColumnSchema::GlobalLedgerCurrency),
            "My Bank Account".to_string(),
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_test_account_header_row() {
        let input = ",My Bank Account,,,,,,,,,,\nbla";
        test_parser(
            input,
            account_header_row(ColumnSchema::PerAccountCurrency),
            "My Bank Account".to_string(),
            "bla",
        );
    }

    #[test]
    fn given_global_schema_test_starting_balance_row() {
        let input = "Starting Balance,,,,,\"$12,345.67\"\nbla";
        test_parser(
            input,
            starting_balance_row(ColumnSchema::GlobalLedgerCurrency),
            Decimal::new(1234567, 2),
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_test_starting_balance_row() {
        let input = "Starting Balance,,,,,\"$12,345.67\",USD,,,,\"$12,345.67\",USD\nbla";
        test_parser(
            input,
            starting_balance_row(ColumnSchema::PerAccountCurrency),
            Decimal::new(1234567, 2),
            "bla",
        );
    }

    #[test]
    fn given_global_schema_test_posting_row_credit() {
        let input = ",2024-01-04,Some description,,$123.45,\"$1,234.56\"\nbla";
        test_parser(
            input,
            posting_row(ColumnSchema::GlobalLedgerCurrency),
            Posting {
                date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                description: "Some description".to_string(),
                debit: Decimal::new(0, 0),
                credit: Decimal::new(12345, 2),
                balance: Decimal::new(123456, 2),
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_test_posting_row_credit() {
        let input = ",2024-01-04,Some description,,$123.45,\"$1,234.56\",USD,,,$123.45,\"$1,234.56\",USD\nbla";
        test_parser(
            input,
            posting_row(ColumnSchema::PerAccountCurrency),
            Posting {
                date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                description: "Some description".to_string(),
                debit: Decimal::new(0, 0),
                credit: Decimal::new(12345, 2),
                balance: Decimal::new(123456, 2),
            },
            "bla",
        );
    }

    #[test]
    fn given_global_schema_test_posting_row_debit() {
        let input = ",2024-02-01,Some description,\"$1,234.56\",,\"$2,345.67\"\nbla";
        test_parser(
            input,
            posting_row(ColumnSchema::GlobalLedgerCurrency),
            Posting {
                date: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
                description: "Some description".to_string(),
                debit: Decimal::new(123456, 2),
                credit: Decimal::new(0, 0),
                balance: Decimal::new(234567, 2),
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_test_posting_row_debit() {
        let input = ",2024-02-01,Some description,\"$1,234.56\",,\"$2,345.67\",USD,,\"$1,234.56\",,\"$2,345.67\",USD\nbla";
        test_parser(
            input,
            posting_row(ColumnSchema::PerAccountCurrency),
            Posting {
                date: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
                description: "Some description".to_string(),
                debit: Decimal::new(123456, 2),
                credit: Decimal::new(0, 0),
                balance: Decimal::new(234567, 2),
            },
            "bla",
        )
    }

    #[test]
    fn given_global_schema_test_ending_balance_row() {
        let input =
            "Totals and Ending Balance,,,\"$123,456.78\",\"$234,567.89\",\"$45,678.90\"\nbla";
        test_parser(
            input,
            ending_balance_row(ColumnSchema::GlobalLedgerCurrency),
            EndingBalance {
                total_debit: Decimal::new(12345678, 2),
                total_credit: Decimal::new(23456789, 2),
                ending_balance: Decimal::new(4567890, 2),
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_test_ending_balance_row() {
        let input =
            "Totals and Ending Balance,,,\"$123,456.78\",\"$234,567.89\",\"$45,678.90\",USD,,\"$123,456.78\",\"$234,567.89\",\"$45,678.90\",USD\nbla";
        test_parser(
            input,
            ending_balance_row(ColumnSchema::PerAccountCurrency),
            EndingBalance {
                total_debit: Decimal::new(12345678, 2),
                total_credit: Decimal::new(23456789, 2),
                ending_balance: Decimal::new(4567890, 2),
            },
            "bla",
        );
    }

    #[test]
    fn given_global_schema_test_balance_change_row() {
        let input = "Balance Change,,,\"$9,876.54\",,\nbla";
        test_parser(
            input,
            balance_change_row(ColumnSchema::GlobalLedgerCurrency),
            Decimal::new(987654, 2),
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_test_balance_change_row() {
        let input = "Balance Change,,,\"$9,876.54\",,,USD,,\"$9,876.54\",,,USD\nbla";
        test_parser(
            input,
            balance_change_row(ColumnSchema::PerAccountCurrency),
            Decimal::new(987654, 2),
            "bla",
        );
    }

    #[test]
    fn given_global_schema_test_account_empty() {
        let input = r#",My Bank Account,,,,
Starting Balance,,,,,$12.34
Totals and Ending Balance,,,$0.00,$0.00,"$12.34"
Balance Change,,,"$0.0",,"#;
        test_parser(
            input,
            account(ColumnSchema::GlobalLedgerCurrency),
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
            },
            "",
        );
    }

    #[test]
    fn given_peraccount_schema_test_account_empty() {
        let input = r#",My Bank Account,,,,,,,,,,
Starting Balance,,,,,$12.34,USD,,,,$12.34,USD
Totals and Ending Balance,,,"$0.00","$0.00","$12.34",USD,,"$0.00","$0.00","$12.34",USD
Balance Change,,,"$0.00",,,USD,,"$0.00",,,USD"#;
        test_parser(
            input,
            account(ColumnSchema::PerAccountCurrency),
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
            },
            "",
        );
    }

    #[test]
    fn given_global_schema_test_account_valid_with_negative_change() {
        let input = r#",Some Account,,,,
Starting Balance,,,,,$123.45
,2024-01-04,Some: Addition,$1.23,,$124.68
,2024-04-04,Some: Withdrawal,,$15.67,$109.01
Totals and Ending Balance,,,$1.23,$15.67,$109.01
Balance Change,,,-$14.44,,"#;
        test_parser(
            input,
            account(ColumnSchema::GlobalLedgerCurrency),
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
            },
            "",
        );
    }

    #[test]
    fn given_peraccount_schema_test_account_valid_with_negative_change() {
        let input = r#",Some Account,,,,,,,,,,
Starting Balance,,,,,$123.45,USD,,,,$123.45,USD
,2024-01-04,Some: Addition,$1.23,,$124.68,USD,,$1.23,,$124.68,USD
,2024-04-04,Some: Withdrawal,,$15.67,$109.01,USD,,,$15.67,$109.01,USD
Totals and Ending Balance,,,$1.23,$15.67,$109.01,USD,,$1.23,$15.67,$109.01,USD
Balance Change,,,-$14.44,,,USD,,-$14.44,,,USD"#;
        test_parser(
            input,
            account(ColumnSchema::PerAccountCurrency),
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
            },
            "",
        );
    }

    #[test]
    fn given_global_schema_test_account_valid_with_positive_change() {
        let input = r#",Some Account,,,,
Starting Balance,,,,,$123.45
,2024-01-04,Some: Withdrawal,,$1.23,$122.22
,2024-04-04,Some: Addition,$15.67,,$137.89
Totals and Ending Balance,,,$15.67,$1.23,$137.89
Balance Change,,,$14.44,,"#;
        test_parser(
            input,
            account(ColumnSchema::GlobalLedgerCurrency),
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
            },
            "",
        )
    }

    #[test]
    fn given_peraccount_schema_test_account_valid_with_positive_change() {
        let input = r#",Some Account,,,,,,,,,,
Starting Balance,,,,,$123.45,USD,,,,$123.45,USD
,2024-01-04,Some: Withdrawal,,$1.23,$122.22,USD,,,$1.23,$122.22,USD
,2024-04-04,Some: Addition,$15.67,,$137.89,USD,,$15.67,,$137.89,USD
Totals and Ending Balance,,,$15.67,$1.23,$137.89,USD,,$15.67,$1.23,$137.89,USD
Balance Change,,,$14.44,,,USD,,$14.44,,,USD"#;
        test_parser(
            input,
            account(ColumnSchema::PerAccountCurrency),
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
            },
            "",
        )
    }
}
