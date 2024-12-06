use chrono::NaiveDate;
use chumsky::{error::Simple, prelude::just, Parser as _};

use super::utils::{cell_tag, comma, date_range, line_any_content, line_tag, row_end};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ColumnSchema {
    /// The CSV has no columns for currency type, all amounts are in USD.
    GlobalLedgerCurrency,

    /// The CSV has separate columns for ledger currency and account currency. An account may have a currency different from USD.
    PerAccountCurrency,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Header {
    pub ledger_name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub column_schema: ColumnSchema,
}

pub fn header() -> impl chumsky::Parser<char, Header, Error = Simple<char>> {
    let date_range_row = just("Date Range: ")
        .ignore_then(date_range())
        .then_ignore(row_end());

    line_tag("Account Transactions")
        .ignore_then(line_any_content())
        .then(date_range_row)
        .then_ignore(line_tag("Report Type: Accrual (Paid & Unpaid)"))
        .then(header_row())
        .map(|((ledger_name, date_range), column_schema)| Header {
            ledger_name,
            start_date: date_range.0,
            end_date: date_range.1,
            column_schema,
        })
        .labelled("header")
}

fn header_row() -> impl chumsky::Parser<char, ColumnSchema, Error = Simple<char>> {
    let header_start = cell_tag("ACCOUNT NUMBER")
        .ignore_then(comma())
        .ignore_then(cell_tag("DATE"))
        .ignore_then(comma())
        .ignore_then(cell_tag("DESCRIPTION"))
        .ignore_then(comma())
        .ignore_then(cell_tag("DEBIT (In Business Currency)"))
        .ignore_then(comma())
        .ignore_then(cell_tag("CREDIT (In Business Currency)"))
        .ignore_then(comma())
        .ignore_then(cell_tag("BALANCE (In Business Currency)"));

    let header_with_ledger_currency = row_end().to(ColumnSchema::GlobalLedgerCurrency);

    let header_with_account_currency = comma()
        .ignore_then(cell_tag("Business Currency"))
        .ignore_then(comma())
        .ignore_then(comma())
        .ignore_then(cell_tag("DEBIT (In Account Currency)"))
        .ignore_then(comma())
        .ignore_then(cell_tag("CREDIT (In Account Currency)"))
        .ignore_then(comma())
        .ignore_then(cell_tag("BALANCE (In Account Currency)"))
        .ignore_then(comma())
        .ignore_then(cell_tag("Account Currency"))
        .ignore_then(row_end())
        .to(ColumnSchema::PerAccountCurrency);

    header_start
        .ignore_then(header_with_ledger_currency.or(header_with_account_currency))
        .labelled("csv header row")
}

#[cfg(test)]
mod tests {
    use crate::import::parser::utils::test_parser;

    use super::*;

    #[test]
    fn test_header_with_global_ledger_currency() {
        let input = r#"Account Transactions
Personal
Date Range: 2024-01-01 to 2024-11-30
Report Type: Accrual (Paid & Unpaid)
ACCOUNT NUMBER,DATE,DESCRIPTION,DEBIT (In Business Currency),CREDIT (In Business Currency),BALANCE (In Business Currency)
,..."#;
        test_parser(
            input,
            header(),
            Header {
                ledger_name: "Personal".to_string(),
                start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
                column_schema: ColumnSchema::GlobalLedgerCurrency,
            },
            ",...",
        );
    }

    #[test]
    fn test_header_with_per_account_currency() {
        let input = r#"Account Transactions
Personal
Date Range: 2024-01-01 to 2024-11-30
Report Type: Accrual (Paid & Unpaid)
ACCOUNT NUMBER,DATE,DESCRIPTION,DEBIT (In Business Currency),CREDIT (In Business Currency),BALANCE (In Business Currency),Business Currency,,DEBIT (In Account Currency),CREDIT (In Account Currency),BALANCE (In Account Currency),Account Currency
,..."#;
        test_parser(
            input,
            header(),
            Header {
                ledger_name: "Personal".to_string(),
                start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
                column_schema: ColumnSchema::PerAccountCurrency,
            },
            ",...",
        );
    }
}
