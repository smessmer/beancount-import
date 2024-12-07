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
use crate::ir::{Amount, LEDGER_CURRENCY, LEDGER_CURRENCY_SYMBOL};

fn currency_symbol(currency: &str) -> Result<char, String> {
    match currency {
        "USD" => Ok('$'),
        "EUR" => Ok('€'),
        _ => Err(format!("Unexpected currency {currency}")),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub account_currency: String,
    pub starting_balance: Amount,
    pub postings: Vec<Posting>,
    pub ending_balance: EndingBalance,
    pub balance_change: Amount,
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
        let mut total_debit = Amount::zero();
        let mut total_credit = Amount::zero();
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
    pub debit: Amount,
    pub credit: Amount,
    pub balance: Amount,
}

impl Posting {
    pub fn amount(&self) -> Result<Amount> {
        ensure!(
            self.debit.is_zero() != self.credit.is_zero(),
            "Exactly one of debit and credit must be zero"
        );
        Ok(self.debit - self.credit)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct EndingBalance {
    pub total_debit: Amount,
    pub total_credit: Amount,
    pub ending_balance: Amount,
}

pub fn account(
    column_schema: ColumnSchema,
) -> impl chumsky::Parser<char, Account, Error = Simple<char>> {
    account_header_row(column_schema)
        .then(
            starting_balance_row(column_schema).then_with(move |starting_balance| {
                posting_row(column_schema, starting_balance.account_currency.clone())
                    .repeated()
                    .then(ending_balance_row(
                        column_schema,
                        starting_balance.account_currency.clone(),
                    ))
                    .then(balance_change_row(
                        column_schema,
                        starting_balance.account_currency.clone(),
                    ))
                    .map(move |((postings, ending_balance), balance_change)| {
                        (
                            starting_balance.clone(),
                            postings,
                            ending_balance,
                            balance_change,
                        )
                    })
            }),
        )
        .try_map(
            |(name, (starting_balance, postings, ending_balance, balance_change)), span| {
                let account_currency = starting_balance.account_currency;
                let account = Account {
                    name,
                    account_currency,
                    starting_balance: starting_balance.starting_balance,
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

#[derive(Clone, PartialEq, Eq, Debug)]
struct StartingBalanceRow {
    starting_balance: Amount,
    account_currency: String,
}

fn starting_balance_row(
    column_schema: ColumnSchema,
) -> impl chumsky::Parser<char, StartingBalanceRow, Error = Simple<char>> {
    let amount_in_ledger_currency = cell_tag("Starting Balance")
        .ignore_then(comma().ignore_then(empty_cell()).repeated().exactly(4))
        .ignore_then(comma())
        .ignore_then(amount_cell());
    let parser = match column_schema {
        ColumnSchema::GlobalLedgerCurrency => amount_in_ledger_currency
            .then_ignore(row_end())
            .try_map(|amount, span| {
                if amount.currency_symbol != LEDGER_CURRENCY_SYMBOL {
                    return Err(Simple::custom(
                        span,
                        format!("Ledger currency symbol is not {LEDGER_CURRENCY}"),
                    ));
                }
                Ok(StartingBalanceRow {
                    starting_balance: Amount {
                        in_ledger_currency: amount.amount,
                        in_account_currency: amount.amount,
                    },
                    account_currency: LEDGER_CURRENCY.to_string(),
                })
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
                    if ledger_currency != LEDGER_CURRENCY {
                        return Err(Simple::custom(
                            span,
                            format!("Ledger currency is not {LEDGER_CURRENCY}"),
                        ));
                    }
                    if amount_in_ledger_currency.currency_symbol != LEDGER_CURRENCY_SYMBOL {
                        return Err(Simple::custom(
                            span,
                            format!("Ledger currency symbol is not {LEDGER_CURRENCY}"),
                        ));
                    }
                    let expected_account_currency_symbol = currency_symbol(&account_currency)
                        .map_err(|err| {
                            Simple::custom(span.clone(), format!("Invalid account currency: {err}"))
                        })?;
                    if amount_in_account_currency.currency_symbol
                        != expected_account_currency_symbol
                    {
                        return Err(Simple::custom(
                            span,
                            format!(
                                "Account currency is {account_currency} but symbol is {}",
                                amount_in_account_currency.currency_symbol
                            ),
                        ));
                    }
                    if account_currency == LEDGER_CURRENCY
                        && amount_in_account_currency.amount != amount_in_ledger_currency.amount
                    {
                        return Err(Simple::custom(
                            span,
                            "Account currency is ledger currency but amounts differ",
                        ));
                    }
                    Ok(StartingBalanceRow {
                        starting_balance: Amount {
                            in_ledger_currency: amount_in_ledger_currency.amount,
                            in_account_currency: amount_in_account_currency.amount,
                        },
                        account_currency,
                    })
                },
            )
            .boxed(),
    };
    parser.labelled("starting balance row")
}

fn posting_row(
    column_schema: ColumnSchema,
    expected_account_currency: String,
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
                    if debit.currency_symbol != LEDGER_CURRENCY_SYMBOL {
                        return Err(Simple::custom(
                            span,
                            format!("Debit currency symbol is not {LEDGER_CURRENCY}"),
                        ));
                    }
                    debit.amount
                }
                None => Decimal::zero(),
            };
            let credit = match credit {
                Some(credit) => {
                    if credit.currency_symbol != LEDGER_CURRENCY_SYMBOL {
                        return Err(Simple::custom(
                            span,
                            format!("Credit currency symbol is not {LEDGER_CURRENCY}"),
                        ));
                    }
                    credit.amount
                }
                None => Decimal::zero(),
            };
            if balance.currency_symbol != LEDGER_CURRENCY_SYMBOL {
                return Err(Simple::custom(
                    span,
                    format!("Balance currency symbol is not {LEDGER_CURRENCY}"),
                ));
            }
            let balance = balance.amount;
            Ok(Posting {
                date,
                description,
                debit: Amount {
                    in_ledger_currency: debit,
                    in_account_currency: debit,
                },
                credit: Amount {
                    in_ledger_currency: credit,
                    in_account_currency: credit,
                },
                balance: Amount {
                    in_ledger_currency: balance,
                    in_account_currency: balance,
                },
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
               move |(
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
                    if ledger_currency != LEDGER_CURRENCY {
                        return Err(Simple::custom(
                            span,
                            format!("Ledger currency is not {LEDGER_CURRENCY}"),
                        ));
                    }
                    if account_currency != expected_account_currency {
                        return Err(Simple::custom(
                            span,
                            format!("Expected account currency '{expected_account_currency}' but got '{account_currency}'"),
                        ));
                    }
                    let expected_account_currency_symbol = currency_symbol(&account_currency)
                        .map_err(|err| {
                            Simple::custom(span.clone(), format!("Invalid account currency: {err}"))
                        })?;
                    let debit_in_account_currency = match debit_in_account_currency {
                        Some(debit) => {
                            if debit.currency_symbol != expected_account_currency_symbol {
                                return Err(Simple::custom(
                                    span,
                                    format!(
                                        "Expected debit currency symbol '{expected_account_currency_symbol}' but got '{}'",
                                        debit.currency_symbol,
                                    ),
                                ));
                            }
                            Some(debit.amount)
                        }
                        None => None,
                    };
                    let credit_in_account_currency = match credit_in_account_currency {
                        Some(credit) => {
                            if credit.currency_symbol != expected_account_currency_symbol {
                                return Err(Simple::custom(
                                    span,
                                    format!(
                                        "Expected credit currency symbol '{expected_account_currency_symbol}' but got '{}'",
                                        credit.currency_symbol,
                                    ),
                                ));
                            }
                            Some(credit.amount)
                        }
                        None => None,
                    };
                    if balance_in_account_currency.currency_symbol != expected_account_currency_symbol {
                        return Err(Simple::custom(
                            span,
                            format!(
                                "Expected balance currency symbol '{expected_account_currency_symbol}' but got '{}'",
                                balance_in_account_currency.currency_symbol,
                            ),
                        ));
                    }
                    let balance_in_account_currency = balance_in_account_currency.amount;
                    if debit_in_account_currency.is_some() == credit_in_account_currency.is_some() {
                        return Err(Simple::custom(
                            span,
                            "Exactly one of debit and credit must be present",
                        ));
                    }
                    let debit_in_account_currency = debit_in_account_currency.unwrap_or_else(Decimal::zero);
                    let credit_in_account_currency = credit_in_account_currency.unwrap_or_else(Decimal::zero);
                    if debit_in_account_currency.is_zero() != posting.debit.in_ledger_currency.is_zero() {
                        return Err(Simple::custom(
                            span,
                            "Debit in account currency must be zero if and only if debit in ledger currency is zero",
                        ));
                    }
                    if credit_in_account_currency.is_zero() != posting.credit.in_ledger_currency.is_zero() {
                        return Err(Simple::custom(
                            span,
                            "Credit in account currency must be zero if and only if credit in ledger currency is zero",
                        ));
                    }
                    if account_currency == LEDGER_CURRENCY && debit_in_account_currency != posting.debit.in_ledger_currency
                    {
                        return Err(Simple::custom(
                            span,
                            "Account currency is ledger currency but debit amounts differ",
                        ));
                    }
                    if account_currency == LEDGER_CURRENCY && credit_in_account_currency != posting.credit.in_ledger_currency
                    {
                        return Err(Simple::custom(
                            span,
                            "Account currency is ledger currency but credit amounts differ",
                        ));
                    }
                    if account_currency == LEDGER_CURRENCY && balance_in_account_currency != posting.balance.in_ledger_currency
                    {
                        return Err(Simple::custom(
                            span,
                            "Account currency is ledger currency but balance amounts differ",
                        ));
                    }
                    Ok(Posting {
                        date: posting.date,
                        description: posting.description,
                        debit: Amount {
                            in_ledger_currency: posting.debit.in_ledger_currency,
                            in_account_currency: debit_in_account_currency,
                        },
                        credit: Amount {
                            in_ledger_currency: posting.credit.in_ledger_currency,
                            in_account_currency: credit_in_account_currency,
                        },
                        balance: Amount {
                            in_ledger_currency: posting.balance.in_ledger_currency,
                            in_account_currency: balance_in_account_currency,
                        },
                    })
                },
            )
            .boxed(),
    };
    parser.labelled("posting row")
}

fn ending_balance_row(
    column_schema: ColumnSchema,
    expected_account_currency: String,
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
            if total_debit.currency_symbol != LEDGER_CURRENCY_SYMBOL {
                return Err(Simple::custom(
                    span,
                    format!("Total debit currency symbol is not {LEDGER_CURRENCY}"),
                ));
            }
            if total_credit.currency_symbol != LEDGER_CURRENCY_SYMBOL {
                return Err(Simple::custom(
                    span,
                    format!("Total credit currency symbol is not {LEDGER_CURRENCY}"),
                ));
            }
            if ending_balance.currency_symbol != LEDGER_CURRENCY_SYMBOL {
                return Err(Simple::custom(
                    span,
                    format!("Ending balance currency symbol is not {LEDGER_CURRENCY}"),
                ));
            }
            Ok(EndingBalance {
                total_debit: Amount {
                    in_ledger_currency: total_debit.amount,
                    in_account_currency: total_debit.amount,
                },
                total_credit: Amount {
                    in_ledger_currency: total_credit.amount,
                    in_account_currency: total_credit.amount,
                },
                ending_balance: Amount {
                    in_ledger_currency: ending_balance.amount,
                    in_account_currency: ending_balance.amount,
                },
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
                move | (
                    ((((ending_balance,
                    ledger_currency),
                    total_debit_in_account_currency),
                    total_credit_in_account_currency),
                    ending_balance_in_account_currency),
                    account_currency,
                ), span
                | {
                    if ledger_currency != LEDGER_CURRENCY {
                        return Err(Simple::custom(
                            span,
                            format!("Ledger currency is not {LEDGER_CURRENCY}"),
                        ));
                    }
                    if account_currency != expected_account_currency {
                        return Err(Simple::custom(
                            span,
                            format!("Expected account currency '{expected_account_currency}' but got '{account_currency}'"),
                        ));
                    }
                    let expected_account_currency= currency_symbol(&account_currency)
                        .map_err(|err| {
                            Simple::custom(span.clone(), format!("Invalid account currency: {err}"))
                        })?;
                    if total_debit_in_account_currency.currency_symbol != expected_account_currency {
                        return Err(Simple::custom(
                            span,
                            format!("Expected total debit currency symbol '{expected_account_currency}' but got '{}'",
                            total_debit_in_account_currency.currency_symbol),
                        ));
                    }
                    if total_credit_in_account_currency.currency_symbol != expected_account_currency {
                        return Err(Simple::custom(
                            span,
                            format!("Expected total credit currency symbol '{expected_account_currency}' but got '{}'",
                            total_credit_in_account_currency.currency_symbol),
                        ));
                    }
                    if ending_balance_in_account_currency.currency_symbol != expected_account_currency {
                        return Err(Simple::custom(
                            span,
                            format!("Expected ending balance currency symbol '{expected_account_currency}' but got '{}'",
                            ending_balance_in_account_currency.currency_symbol),
                        ));
                    }
                    if account_currency == LEDGER_CURRENCY && total_debit_in_account_currency.amount != ending_balance.total_debit.in_ledger_currency {
                        return Err(Simple::custom(
                            span,
                            "Account currency is ledger currency but total debit amounts differ",
                        ));
                    }
                    if account_currency == LEDGER_CURRENCY && total_credit_in_account_currency.amount != ending_balance.total_credit.in_ledger_currency {
                        return Err(Simple::custom(
                            span,
                            "Account currency is ledger currency but total credit amounts differ",
                        ));
                    }
                    if account_currency == LEDGER_CURRENCY && ending_balance_in_account_currency.amount != ending_balance.ending_balance.in_ledger_currency {
                        return Err(Simple::custom(
                            span,
                            "Account currency is ledger currency but ending balance amounts differ",
                        ));
                    }
                    Ok(EndingBalance {
                        total_debit: Amount {
                            in_ledger_currency: ending_balance.total_debit.in_ledger_currency,
                            in_account_currency: total_debit_in_account_currency.amount,
                        },
                        total_credit: Amount {
                            in_ledger_currency: ending_balance.total_credit.in_ledger_currency,
                            in_account_currency: total_credit_in_account_currency.amount,
                        },
                        ending_balance: Amount {
                            in_ledger_currency: ending_balance.ending_balance.in_ledger_currency,
                            in_account_currency: ending_balance_in_account_currency.amount,
                        },
                    })
                },
        ).boxed()
    };
    parser.labelled("ending balance row")
}

fn balance_change_row(
    column_schema: ColumnSchema,
    expected_account_currency: String,
) -> impl chumsky::Parser<char, Amount, Error = Simple<char>> {
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
        .then_ignore(empty_cell())
        .try_map(|amount, span| {
            if amount.currency_symbol != LEDGER_CURRENCY_SYMBOL {
                return Err(Simple::custom(span, "Currency symbol is not $"));
            }
            Ok(Amount {
                in_ledger_currency: amount.amount,
                in_account_currency: amount.amount,
            })
        });
    let parser = match column_schema {
        ColumnSchema::GlobalLedgerCurrency =>
            common_rows.then_ignore(row_end()).boxed(),
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
                .try_map(move |(
                    ((balance_change, ledger_currency), balance_change_in_account_currency),
                    account_currency,
                ), span| {
                    if ledger_currency != LEDGER_CURRENCY {
                        return Err(Simple::custom(
                            span,
                            format!("Ledger currency is not {LEDGER_CURRENCY}"),
                        ));
                    }
                    if account_currency != expected_account_currency {
                        return Err(Simple::custom(
                            span,
                            format!("Expected account currency '{expected_account_currency}' but got '{account_currency}'"),
                        ));
                    }
                    let expected_account_currency_symbol = currency_symbol(&account_currency)
                        .map_err(|err| {
                            Simple::custom(span.clone(), format!("Invalid account currency: {err}"))
                        })?;
                    if balance_change_in_account_currency.currency_symbol != expected_account_currency_symbol {
                        return Err(Simple::custom(
                            span,
                            format!("Expected balance change currency symbol '{expected_account_currency_symbol}' but got '{}'",
                            balance_change_in_account_currency.currency_symbol),
                        ));
                    }
                    if account_currency == LEDGER_CURRENCY && balance_change_in_account_currency.amount != balance_change.in_ledger_currency {
                        return Err(Simple::custom(
                            span,
                            "Account currency is ledger currency but balance change amounts differ",
                        ));
                    }
                    Ok(Amount {
                        in_ledger_currency: balance_change.in_ledger_currency,
                        in_account_currency: balance_change_in_account_currency.amount,
                    })
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
            StartingBalanceRow {
                account_currency: LEDGER_CURRENCY.to_string(),
                starting_balance: Amount {
                    in_ledger_currency: Decimal::new(1234567, 2),
                    in_account_currency: Decimal::new(1234567, 2),
                },
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_same_currency_test_starting_balance_row() {
        let input = "Starting Balance,,,,,\"$12,345.67\",USD,,,,\"$12,345.67\",USD\nbla";
        test_parser(
            input,
            starting_balance_row(ColumnSchema::PerAccountCurrency),
            StartingBalanceRow {
                account_currency: "USD".to_string(),
                starting_balance: Amount {
                    in_ledger_currency: Decimal::new(1234567, 2),
                    in_account_currency: Decimal::new(1234567, 2),
                },
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_different_currency_test_starting_balance_row() {
        let input = "Starting Balance,,,,,\"$12,345.67\",USD,,,,\"€13,345.67\",EUR\nbla";
        test_parser(
            input,
            starting_balance_row(ColumnSchema::PerAccountCurrency),
            StartingBalanceRow {
                account_currency: "EUR".to_string(),
                starting_balance: Amount {
                    in_ledger_currency: Decimal::new(1234567, 2),
                    in_account_currency: Decimal::new(1334567, 2),
                },
            },
            "bla",
        );
    }

    #[test]
    fn given_global_schema_test_posting_row_credit() {
        let input = ",2024-01-04,Some description,,$123.45,\"$1,234.56\"\nbla";
        test_parser(
            input,
            posting_row(
                ColumnSchema::GlobalLedgerCurrency,
                LEDGER_CURRENCY.to_string(),
            ),
            Posting {
                date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                description: "Some description".to_string(),
                debit: Amount {
                    in_ledger_currency: Decimal::new(0, 0),
                    in_account_currency: Decimal::new(0, 0),
                },
                credit: Amount {
                    in_ledger_currency: Decimal::new(12345, 2),
                    in_account_currency: Decimal::new(12345, 2),
                },
                balance: Amount {
                    in_ledger_currency: Decimal::new(123456, 2),
                    in_account_currency: Decimal::new(123456, 2),
                },
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_same_currency_test_posting_row_credit() {
        let input = ",2024-01-04,Some description,,$123.45,\"$1,234.56\",USD,,,$123.45,\"$1,234.56\",USD\nbla";
        test_parser(
            input,
            posting_row(ColumnSchema::PerAccountCurrency, "USD".to_string()),
            Posting {
                date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                description: "Some description".to_string(),
                debit: Amount {
                    in_ledger_currency: Decimal::new(0, 0),
                    in_account_currency: Decimal::new(0, 0),
                },
                credit: Amount {
                    in_ledger_currency: Decimal::new(12345, 2),
                    in_account_currency: Decimal::new(12345, 2),
                },
                balance: Amount {
                    in_ledger_currency: Decimal::new(123456, 2),
                    in_account_currency: Decimal::new(123456, 2),
                },
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_different_currency_test_posting_row_credit() {
        let input = ",2024-01-04,Some description,,$123.45,\"$1,234.56\",USD,,,€223.45,\"€2,234.56\",EUR\nbla";
        test_parser(
            input,
            posting_row(ColumnSchema::PerAccountCurrency, "EUR".to_string()),
            Posting {
                date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                description: "Some description".to_string(),
                debit: Amount {
                    in_ledger_currency: Decimal::new(0, 0),
                    in_account_currency: Decimal::new(0, 0),
                },
                credit: Amount {
                    in_ledger_currency: Decimal::new(12345, 2),
                    in_account_currency: Decimal::new(22345, 2),
                },
                balance: Amount {
                    in_ledger_currency: Decimal::new(123456, 2),
                    in_account_currency: Decimal::new(223456, 2),
                },
            },
            "bla",
        );
    }

    #[test]
    fn given_global_schema_test_posting_row_debit() {
        let input = ",2024-02-01,Some description,\"$1,234.56\",,\"$2,345.67\"\nbla";
        test_parser(
            input,
            posting_row(
                ColumnSchema::GlobalLedgerCurrency,
                LEDGER_CURRENCY.to_string(),
            ),
            Posting {
                date: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
                description: "Some description".to_string(),
                debit: Amount {
                    in_ledger_currency: Decimal::new(123456, 2),
                    in_account_currency: Decimal::new(123456, 2),
                },
                credit: Amount {
                    in_ledger_currency: Decimal::new(0, 0),
                    in_account_currency: Decimal::new(0, 0),
                },
                balance: Amount {
                    in_ledger_currency: Decimal::new(234567, 2),
                    in_account_currency: Decimal::new(234567, 2),
                },
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_same_currency_test_posting_row_debit() {
        let input = ",2024-02-01,Some description,\"$1,234.56\",,\"$2,345.67\",USD,,\"$1,234.56\",,\"$2,345.67\",USD\nbla";
        test_parser(
            input,
            posting_row(ColumnSchema::PerAccountCurrency, "USD".to_string()),
            Posting {
                date: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
                description: "Some description".to_string(),
                debit: Amount {
                    in_ledger_currency: Decimal::new(123456, 2),
                    in_account_currency: Decimal::new(123456, 2),
                },
                credit: Amount {
                    in_ledger_currency: Decimal::new(0, 0),
                    in_account_currency: Decimal::new(0, 0),
                },
                balance: Amount {
                    in_ledger_currency: Decimal::new(234567, 2),
                    in_account_currency: Decimal::new(234567, 2),
                },
            },
            "bla",
        )
    }

    #[test]
    fn given_peraccount_schema_different_currency_test_posting_row_debit() {
        let input = ",2024-02-01,Some description,\"$1,234.56\",,\"$2,345.67\",USD,,\"€2,234.56\",,\"€3,345.67\",EUR\nbla";
        test_parser(
            input,
            posting_row(ColumnSchema::PerAccountCurrency, "EUR".to_string()),
            Posting {
                date: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
                description: "Some description".to_string(),
                debit: Amount {
                    in_ledger_currency: Decimal::new(123456, 2),
                    in_account_currency: Decimal::new(223456, 2),
                },
                credit: Amount {
                    in_ledger_currency: Decimal::new(0, 0),
                    in_account_currency: Decimal::new(0, 0),
                },
                balance: Amount {
                    in_ledger_currency: Decimal::new(234567, 2),
                    in_account_currency: Decimal::new(334567, 2),
                },
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
            ending_balance_row(
                ColumnSchema::GlobalLedgerCurrency,
                LEDGER_CURRENCY.to_string(),
            ),
            EndingBalance {
                total_debit: Amount {
                    in_ledger_currency: Decimal::new(12345678, 2),
                    in_account_currency: Decimal::new(12345678, 2),
                },
                total_credit: Amount {
                    in_ledger_currency: Decimal::new(23456789, 2),
                    in_account_currency: Decimal::new(23456789, 2),
                },
                ending_balance: Amount {
                    in_ledger_currency: Decimal::new(4567890, 2),
                    in_account_currency: Decimal::new(4567890, 2),
                },
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_same_currency_test_ending_balance_row() {
        let input =
                "Totals and Ending Balance,,,\"$123,456.78\",\"$234,567.89\",\"$45,678.90\",USD,,\"$123,456.78\",\"$234,567.89\",\"$45,678.90\",USD\nbla";
        test_parser(
            input,
            ending_balance_row(ColumnSchema::PerAccountCurrency, "USD".to_string()),
            EndingBalance {
                total_debit: Amount {
                    in_ledger_currency: Decimal::new(12345678, 2),
                    in_account_currency: Decimal::new(12345678, 2),
                },
                total_credit: Amount {
                    in_ledger_currency: Decimal::new(23456789, 2),
                    in_account_currency: Decimal::new(23456789, 2),
                },
                ending_balance: Amount {
                    in_ledger_currency: Decimal::new(4567890, 2),
                    in_account_currency: Decimal::new(4567890, 2),
                },
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_different_currency_test_ending_balance_row() {
        let input =
                "Totals and Ending Balance,,,\"$123,456.78\",\"$234,567.89\",\"$45,678.90\",USD,,\"€223,456.78\",\"€334,567.89\",\"€55,678.90\",EUR\nbla";
        test_parser(
            input,
            ending_balance_row(ColumnSchema::PerAccountCurrency, "EUR".to_string()),
            EndingBalance {
                total_debit: Amount {
                    in_ledger_currency: Decimal::new(12345678, 2),
                    in_account_currency: Decimal::new(22345678, 2),
                },
                total_credit: Amount {
                    in_ledger_currency: Decimal::new(23456789, 2),
                    in_account_currency: Decimal::new(33456789, 2),
                },
                ending_balance: Amount {
                    in_ledger_currency: Decimal::new(4567890, 2),
                    in_account_currency: Decimal::new(5567890, 2),
                },
            },
            "bla",
        );
    }

    #[test]
    fn given_global_schema_test_balance_change_row() {
        let input = "Balance Change,,,\"$9,876.54\",,\nbla";
        test_parser(
            input,
            balance_change_row(
                ColumnSchema::GlobalLedgerCurrency,
                LEDGER_CURRENCY.to_string(),
            ),
            Amount {
                in_ledger_currency: Decimal::new(987654, 2),
                in_account_currency: Decimal::new(987654, 2),
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_same_currency_test_balance_change_row() {
        let input = "Balance Change,,,\"$9,876.54\",,,USD,,\"$9,876.54\",,,USD\nbla";
        test_parser(
            input,
            balance_change_row(ColumnSchema::PerAccountCurrency, "USD".to_string()),
            Amount {
                in_ledger_currency: Decimal::new(987654, 2),
                in_account_currency: Decimal::new(987654, 2),
            },
            "bla",
        );
    }

    #[test]
    fn given_peraccount_schema_different_currency_test_balance_change_row() {
        let input = "Balance Change,,,\"$9,876.54\",,,USD,,\"€1,876.54\",,,EUR\nbla";
        test_parser(
            input,
            balance_change_row(ColumnSchema::PerAccountCurrency, "EUR".to_string()),
            Amount {
                in_ledger_currency: Decimal::new(987654, 2),
                in_account_currency: Decimal::new(187654, 2),
            },
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
                account_currency: LEDGER_CURRENCY.to_string(),
                starting_balance: Amount {
                    in_ledger_currency: Decimal::new(1234, 2),
                    in_account_currency: Decimal::new(1234, 2),
                },
                postings: vec![],
                ending_balance: EndingBalance {
                    total_debit: Amount {
                        in_ledger_currency: Decimal::zero(),
                        in_account_currency: Decimal::zero(),
                    },
                    total_credit: Amount {
                        in_ledger_currency: Decimal::zero(),
                        in_account_currency: Decimal::zero(),
                    },
                    ending_balance: Amount {
                        in_ledger_currency: Decimal::new(1234, 2),
                        in_account_currency: Decimal::new(1234, 2),
                    },
                },
                balance_change: Amount {
                    in_ledger_currency: Decimal::zero(),
                    in_account_currency: Decimal::zero(),
                },
            },
            "",
        );
    }

    #[test]
    fn given_peraccount_schema_same_currency_test_account_empty() {
        let input = r#",My Bank Account,,,,,,,,,,
Starting Balance,,,,,$12.34,USD,,,,$12.34,USD
Totals and Ending Balance,,,"$0.00","$0.00","$12.34",USD,,"$0.00","$0.00","$12.34",USD
Balance Change,,,"$0.00",,,USD,,"$0.00",,,USD"#;
        test_parser(
            input,
            account(ColumnSchema::PerAccountCurrency),
            Account {
                name: "My Bank Account".to_string(),
                account_currency: "USD".to_string(),
                starting_balance: Amount {
                    in_ledger_currency: Decimal::new(1234, 2),
                    in_account_currency: Decimal::new(1234, 2),
                },
                postings: vec![],
                ending_balance: EndingBalance {
                    total_debit: Amount {
                        in_ledger_currency: Decimal::zero(),
                        in_account_currency: Decimal::zero(),
                    },
                    total_credit: Amount {
                        in_ledger_currency: Decimal::zero(),
                        in_account_currency: Decimal::zero(),
                    },
                    ending_balance: Amount {
                        in_ledger_currency: Decimal::new(1234, 2),
                        in_account_currency: Decimal::new(1234, 2),
                    },
                },
                balance_change: Amount {
                    in_ledger_currency: Decimal::zero(),
                    in_account_currency: Decimal::zero(),
                },
            },
            "",
        );
    }

    #[test]
    fn given_peraccount_schema_different_currency_test_account_empty() {
        let input = r#",My Bank Account,,,,,,,,,,
Starting Balance,,,,,$12.34,USD,,,,€22.34,EUR
Totals and Ending Balance,,,"$0.00","$0.00","$12.34",USD,,"€0.00","€0.00","€22.34",EUR
Balance Change,,,"$0.00",,,USD,,"€0.00",,,EUR"#;
        test_parser(
            input,
            account(ColumnSchema::PerAccountCurrency),
            Account {
                name: "My Bank Account".to_string(),
                account_currency: "EUR".to_string(),
                starting_balance: Amount {
                    in_ledger_currency: Decimal::new(1234, 2),
                    in_account_currency: Decimal::new(2234, 2),
                },
                postings: vec![],
                ending_balance: EndingBalance {
                    total_debit: Amount {
                        in_ledger_currency: Decimal::zero(),
                        in_account_currency: Decimal::zero(),
                    },
                    total_credit: Amount {
                        in_ledger_currency: Decimal::zero(),
                        in_account_currency: Decimal::zero(),
                    },
                    ending_balance: Amount {
                        in_ledger_currency: Decimal::new(1234, 2),
                        in_account_currency: Decimal::new(2234, 2),
                    },
                },
                balance_change: Amount {
                    in_ledger_currency: Decimal::zero(),
                    in_account_currency: Decimal::zero(),
                },
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
                account_currency: LEDGER_CURRENCY.to_string(),
                starting_balance: Amount {
                    in_ledger_currency: Decimal::new(12345, 2),
                    in_account_currency: Decimal::new(12345, 2),
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
            "",
        );
    }

    #[test]
    fn given_peraccount_schema_same_currency_test_account_valid_with_negative_change() {
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
                account_currency: "USD".to_string(),
                starting_balance: Amount {
                    in_ledger_currency: Decimal::new(12345, 2),
                    in_account_currency: Decimal::new(12345, 2),
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
            "",
        );
    }

    #[test]
    fn given_peraccount_schema_different_currency_test_account_valid_with_negative_change() {
        let input = r#",Some Account,,,,,,,,,,
Starting Balance,,,,,$123.45,USD,,,,€223.45,EUR
,2024-01-04,Some: Addition,$1.23,,$124.68,USD,,€2.23,,€225.68,EUR
,2024-04-04,Some: Withdrawal,,$15.67,$109.01,USD,,,€25.67,€200.01,EUR
Totals and Ending Balance,,,$1.23,$15.67,$109.01,USD,,€2.23,€25.67,€200.01,EUR
Balance Change,,,-$14.44,,,USD,,-€23.44,,,EUR"#;
        test_parser(
            input,
            account(ColumnSchema::PerAccountCurrency),
            Account {
                name: "Some Account".to_string(),
                account_currency: "EUR".to_string(),
                starting_balance: Amount {
                    in_ledger_currency: Decimal::new(12345, 2),
                    in_account_currency: Decimal::new(22345, 2),
                },
                postings: vec![
                    Posting {
                        date: NaiveDate::from_ymd_opt(2024, 1, 4).unwrap(),
                        description: "Some: Addition".to_string(),
                        debit: Amount {
                            in_ledger_currency: Decimal::new(123, 2),
                            in_account_currency: Decimal::new(223, 2),
                        },
                        credit: Amount {
                            in_ledger_currency: Decimal::zero(),
                            in_account_currency: Decimal::zero(),
                        },
                        balance: Amount {
                            in_ledger_currency: Decimal::new(12468, 2),
                            in_account_currency: Decimal::new(22568, 2),
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
                            in_account_currency: Decimal::new(2567, 2),
                        },
                        balance: Amount {
                            in_ledger_currency: Decimal::new(10901, 2),
                            in_account_currency: Decimal::new(20001, 2),
                        },
                    },
                ],
                ending_balance: EndingBalance {
                    total_debit: Amount {
                        in_ledger_currency: Decimal::new(123, 2),
                        in_account_currency: Decimal::new(223, 2),
                    },
                    total_credit: Amount {
                        in_ledger_currency: Decimal::new(1567, 2),
                        in_account_currency: Decimal::new(2567, 2),
                    },
                    ending_balance: Amount {
                        in_ledger_currency: Decimal::new(10901, 2),
                        in_account_currency: Decimal::new(20001, 2),
                    },
                },
                balance_change: Amount {
                    in_ledger_currency: Decimal::new(-1444, 2),
                    in_account_currency: Decimal::new(-2344, 2),
                },
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
            "",
        )
    }

    #[test]
    fn given_peraccount_schema_same_currency_test_account_valid_with_positive_change() {
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
                account_currency: "USD".to_string(),
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
            "",
        )
    }

    #[test]
    fn given_peraccount_schema_different_currency_test_account_valid_with_positive_change() {
        let input = r#",Some Account,,,,,,,,,,
Starting Balance,,,,,$123.45,USD,,,,€223.45,EUR
,2024-01-04,Some: Withdrawal,,$1.23,$122.22,USD,,,€2.23,€221.22,EUR
,2024-04-04,Some: Addition,$15.67,,$137.89,USD,,€25.67,,€246.89,EUR
Totals and Ending Balance,,,$15.67,$1.23,$137.89,USD,,€25.67,€2.23,€246.89,EUR
Balance Change,,,$14.44,,,USD,,€23.44,,,EUR"#;
        test_parser(
            input,
            account(ColumnSchema::PerAccountCurrency),
            Account {
                name: "Some Account".to_string(),
                account_currency: "EUR".to_string(),
                starting_balance: Amount {
                    in_ledger_currency: Decimal::new(12345, 2),
                    in_account_currency: Decimal::new(22345, 2),
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
                            in_account_currency: Decimal::new(223, 2),
                        },
                        balance: Amount {
                            in_ledger_currency: Decimal::new(12222, 2),
                            in_account_currency: Decimal::new(22122, 2),
                        },
                    },
                    Posting {
                        date: NaiveDate::from_ymd_opt(2024, 4, 4).unwrap(),
                        description: "Some: Addition".to_string(),
                        debit: Amount {
                            in_ledger_currency: Decimal::new(1567, 2),
                            in_account_currency: Decimal::new(2567, 2),
                        },
                        credit: Amount {
                            in_ledger_currency: Decimal::zero(),
                            in_account_currency: Decimal::zero(),
                        },
                        balance: Amount {
                            in_ledger_currency: Decimal::new(13789, 2),
                            in_account_currency: Decimal::new(24689, 2),
                        },
                    },
                ],
                ending_balance: EndingBalance {
                    total_debit: Amount {
                        in_ledger_currency: Decimal::new(1567, 2),
                        in_account_currency: Decimal::new(2567, 2),
                    },
                    total_credit: Amount {
                        in_ledger_currency: Decimal::new(123, 2),
                        in_account_currency: Decimal::new(223, 2),
                    },
                    ending_balance: Amount {
                        in_ledger_currency: Decimal::new(13789, 2),
                        in_account_currency: Decimal::new(24689, 2),
                    },
                },
                balance_change: Amount {
                    in_ledger_currency: Decimal::new(1444, 2),
                    in_account_currency: Decimal::new(2344, 2),
                },
            },
            "",
        )
    }
}
