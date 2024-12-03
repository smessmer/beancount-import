use std::num::ParseIntError;

use chrono::NaiveDate;
use nom::{
    bytes::complete::tag,
    character::complete::digit1,
    combinator::map_res,
    error::{context, VerboseError},
    sequence::{separated_pair, tuple},
    IResult, Parser,
};

use super::cell;

pub fn date(input: &str) -> IResult<&str, NaiveDate, VerboseError<&str>> {
    let digits = |expected_len: usize| {
        map_res(digit1, move |parsed: &str| {
            if parsed.len() == expected_len {
                parsed
                    .parse()
                    .map_err(|_: ParseIntError| "Failed to parse integer")
            } else {
                Err("Invalid number of digits")
            }
        })
    };
    context(
        "Failed to parse date",
        map_res(
            tuple((digits(4), tag("-"), digits(2), tag("-"), digits(2))),
            |(year, _, month, _, day)| {
                NaiveDate::from_ymd_opt(year, month as u32, day as u32)
                    .ok_or_else(|| "Invalid date")
            },
        ),
    )
    .parse(input)
}

pub fn date_range(input: &str) -> IResult<&str, (NaiveDate, NaiveDate), VerboseError<&str>> {
    context(
        "Failed to parse date range",
        separated_pair(date, tag(" to "), date),
    )(input)
}

pub fn date_cell(input: &str) -> IResult<&str, NaiveDate, VerboseError<&str>> {
    context(
        "Failed to parse date cell",
        map_res(cell, move |cell_content| {
            date(&cell_content)
                .map(|(_, date)| date)
                .map_err(|_| "Invalid date")
        }),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date() {
        assert_eq!(
            date("2021-01-01"),
            Ok(("", NaiveDate::from_ymd_opt(2021, 1, 1).unwrap()))
        );
        assert_eq!(
            date("2021-01-31"),
            Ok(("", NaiveDate::from_ymd_opt(2021, 1, 31).unwrap()))
        );
        assert_eq!(
            date("2021-02-28"),
            Ok(("", NaiveDate::from_ymd_opt(2021, 2, 28).unwrap()))
        );
        assert_eq!(
            date("2021-02-29"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "2021-02-29",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "2021-02-29",
                        nom::error::VerboseErrorKind::Context("Failed to parse date")
                    )
                ]
            }))
        );
        assert_eq!(
            date("2021-12-31"),
            Ok(("", NaiveDate::from_ymd_opt(2021, 12, 31).unwrap()))
        );
        assert_eq!(
            date("1980-05-14"),
            Ok(("", NaiveDate::from_ymd_opt(1980, 5, 14).unwrap()))
        );
        assert_eq!(
            date("1980-05-32"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "1980-05-32",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "1980-05-32",
                        nom::error::VerboseErrorKind::Context("Failed to parse date")
                    )
                ]
            }))
        );
        assert_eq!(
            date("1980-13-14"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "1980-13-14",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "1980-13-14",
                        nom::error::VerboseErrorKind::Context("Failed to parse date")
                    )
                ]
            }))
        );
        assert_eq!(
            date("1980-00-14"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "1980-00-14",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "1980-00-14",
                        nom::error::VerboseErrorKind::Context("Failed to parse date")
                    )
                ]
            }))
        );
        assert_eq!(
            date("1980-05-00"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "1980-05-00",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "1980-05-00",
                        nom::error::VerboseErrorKind::Context("Failed to parse date")
                    )
                ]
            }))
        );
        assert_eq!(
            date("1980-5-14"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "5-14",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "1980-5-14",
                        nom::error::VerboseErrorKind::Context("Failed to parse date")
                    )
                ]
            }))
        );
        assert_eq!(
            date("1980-05-5"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "5",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "1980-05-5",
                        nom::error::VerboseErrorKind::Context("Failed to parse date")
                    )
                ]
            }))
        );
    }

    #[test]
    fn test_date_range() {
        assert_eq!(
            date_range("2021-01-01 to 2021-01-31"),
            Ok((
                "",
                (
                    NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                    NaiveDate::from_ymd_opt(2021, 1, 31).unwrap()
                )
            ))
        );
        assert_eq!(
            date_range("2021-01-01 to 2021-12-31"),
            Ok((
                "",
                (
                    NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                    NaiveDate::from_ymd_opt(2021, 12, 31).unwrap()
                )
            ))
        );
        assert_eq!(
            date_range("2021-01-01 to 2021-12-31 "),
            Ok((
                " ",
                (
                    NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                    NaiveDate::from_ymd_opt(2021, 12, 31).unwrap()
                )
            ))
        );
        assert_eq!(
            date_range("2021-01-01 to 2021-12-31\n"),
            Ok((
                "\n",
                (
                    NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                    NaiveDate::from_ymd_opt(2021, 12, 31).unwrap()
                )
            ))
        );
        assert_eq!(
            date_range("2021-01-01 to 2021-12-31\n "),
            Ok((
                "\n ",
                (
                    NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                    NaiveDate::from_ymd_opt(2021, 12, 31).unwrap()
                )
            ))
        );
        assert_eq!(
            date_range("2021-01-01 to 2021-12-31\n\n"),
            Ok((
                "\n\n",
                (
                    NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                    NaiveDate::from_ymd_opt(2021, 12, 31).unwrap()
                )
            ))
        );
    }

    #[test]
    fn test_date_cell() {
        assert_eq!(
            date_cell("2021-01-01"),
            Ok(("", NaiveDate::from_ymd_opt(2021, 1, 1).unwrap()))
        );
        assert_eq!(
            date_cell("2021-01-01,"),
            Ok((",", NaiveDate::from_ymd_opt(2021, 1, 1).unwrap()))
        );
        assert_eq!(
            date_cell("2021-01-01,foo"),
            Ok((",foo", NaiveDate::from_ymd_opt(2021, 1, 1).unwrap()))
        );
        assert_eq!(
            date_cell("2021-01-01\nfoo"),
            Ok(("\nfoo", NaiveDate::from_ymd_opt(2021, 1, 1).unwrap()))
        );
        assert_eq!(
            date_cell("2021-01-01\rfoo"),
            Ok(("\rfoo", NaiveDate::from_ymd_opt(2021, 1, 1).unwrap()))
        );

        assert_eq!(
            date_cell("\"2021-01-01\""),
            Ok(("", NaiveDate::from_ymd_opt(2021, 1, 1).unwrap()))
        );
        assert_eq!(
            date_cell("\"2021-01-01\","),
            Ok((",", NaiveDate::from_ymd_opt(2021, 1, 1).unwrap()))
        );
        assert_eq!(
            date_cell("\"2021-01-01\",foo"),
            Ok((",foo", NaiveDate::from_ymd_opt(2021, 1, 1).unwrap()))
        );
        assert_eq!(
            date_cell("\"2021-01-01\"\nfoo"),
            Ok(("\nfoo", NaiveDate::from_ymd_opt(2021, 1, 1).unwrap()))
        );
        assert_eq!(
            date_cell("\"2021-01-01\"\rfoo"),
            Ok(("\rfoo", NaiveDate::from_ymd_opt(2021, 1, 1).unwrap()))
        );
    }
}
