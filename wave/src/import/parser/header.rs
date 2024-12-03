use chrono::NaiveDate;
use nom::{
    bytes::complete::tag,
    combinator::value,
    error::{context, VerboseError},
    sequence::{delimited, tuple},
    IResult,
};

use super::utils::{cell_tag, comma, date_range, line_any_content, line_tag, row_end};

#[derive(Debug, PartialEq, Eq)]
pub struct Header<'a> {
    pub ledger_name: &'a str,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

pub fn header(input: &str) -> IResult<&str, Header<'_>, VerboseError<&str>> {
    let (input, _) = line_tag("Account Transactions")(input)?;
    let (input, ledger_name) = line_any_content(input)?;
    let (input, date_range) = delimited(tag("Date Range: "), date_range, row_end)(input)?;
    let (input, _) = line_tag("Report Type: Accrual (Paid & Unpaid)")(input)?;
    let (input, _) = header_row(input)?;

    Ok((
        input,
        Header {
            ledger_name: ledger_name,
            start_date: date_range.0,
            end_date: date_range.1,
        },
    ))
}

fn header_row(input: &str) -> IResult<&str, (), VerboseError<&str>> {
    context(
        "Failed to parse header_row",
        value(
            (),
            tuple((
                cell_tag("ACCOUNT NUMBER"),
                comma,
                cell_tag("DATE"),
                comma,
                cell_tag("DESCRIPTION"),
                comma,
                cell_tag("DEBIT (In Business Currency)"),
                comma,
                cell_tag("CREDIT (In Business Currency)"),
                comma,
                cell_tag("BALANCE (In Business Currency)"),
                row_end,
            )),
        ),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header() {
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
                    ledger_name: "Personal",
                    start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                    end_date: NaiveDate::from_ymd_opt(2024, 11, 30).unwrap(),
                },
            ))
        );
    }
}
