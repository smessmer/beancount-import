use chrono::NaiveDate;
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::value,
    error::{context, VerboseError},
    sequence::{delimited, preceded, tuple},
    IResult,
};

use super::utils::{
    cell_tag, chumsky_to_nom, comma, date_range, line_any_content, line_tag, row_end,
};

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

pub fn header(input: &str) -> IResult<&str, Header, VerboseError<&str>> {
    let (input, _) = chumsky_to_nom(line_tag("Account Transactions"))(input)?;
    let (input, ledger_name) = chumsky_to_nom(line_any_content())(input)?;
    let (input, date_range) = delimited(
        tag("Date Range: "),
        chumsky_to_nom(date_range()),
        chumsky_to_nom(row_end()),
    )(input)?;
    let (input, _) = chumsky_to_nom(line_tag("Report Type: Accrual (Paid & Unpaid)"))(input)?;
    let (input, column_schema) = header_row(input)?;

    Ok((
        input,
        Header {
            ledger_name: ledger_name,
            start_date: date_range.0,
            end_date: date_range.1,
            column_schema,
        },
    ))
}

fn header_row(input: &str) -> IResult<&str, ColumnSchema, VerboseError<&str>> {
    let header_start = tuple((
        chumsky_to_nom(cell_tag("ACCOUNT NUMBER")),
        chumsky_to_nom(comma()),
        chumsky_to_nom(cell_tag("DATE")),
        chumsky_to_nom(comma()),
        chumsky_to_nom(cell_tag("DESCRIPTION")),
        chumsky_to_nom(comma()),
        chumsky_to_nom(cell_tag("DEBIT (In Business Currency)")),
        chumsky_to_nom(comma()),
        chumsky_to_nom(cell_tag("CREDIT (In Business Currency)")),
        chumsky_to_nom(comma()),
        chumsky_to_nom(cell_tag("BALANCE (In Business Currency)")),
    ));
    let header_with_leger_currency = value(
        ColumnSchema::GlobalLedgerCurrency,
        chumsky_to_nom(row_end()),
    );
    let header_with_account_currency = value(
        ColumnSchema::PerAccountCurrency,
        tuple((
            chumsky_to_nom(comma()),
            chumsky_to_nom(cell_tag("Business Currency")),
            chumsky_to_nom(comma()),
            chumsky_to_nom(comma()),
            chumsky_to_nom(cell_tag("DEBIT (In Account Currency)")),
            chumsky_to_nom(comma()),
            chumsky_to_nom(cell_tag("CREDIT (In Account Currency)")),
            chumsky_to_nom(comma()),
            chumsky_to_nom(cell_tag("BALANCE (In Account Currency)")),
            chumsky_to_nom(comma()),
            chumsky_to_nom(cell_tag("Account Currency")),
            chumsky_to_nom(row_end()),
        )),
    );
    context(
        "Failed to parse header_row",
        preceded(
            header_start,
            alt((header_with_leger_currency, header_with_account_currency)),
        ),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_with_global_ledger_currency() {
        let input = r#"Account Transactions
Personal
Date Range: 2024-01-01 to 2024-11-30
Report Type: Accrual (Paid & Unpaid)
ACCOUNT NUMBER,DATE,DESCRIPTION,DEBIT (In Business Currency),CREDIT (In Business Currency),BALANCE (In Business Currency)
,..."#;
        assert_eq!(
            header(input),
            Ok((
                ",...",
                Header {
                    ledger_name: "Personal".to_string(),
                    start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                    end_date: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
                    column_schema: ColumnSchema::GlobalLedgerCurrency,
                },
            ))
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
        assert_eq!(
            header(input),
            Ok((
                ",...",
                Header {
                    ledger_name: "Personal".to_string(),
                    start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                    end_date: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
                    column_schema: ColumnSchema::PerAccountCurrency,
                },
            ))
        );
    }
}
