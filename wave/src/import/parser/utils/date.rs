use std::{ops::Range, str::FromStr};

use chrono::NaiveDate;
use chumsky::{
    error::Simple,
    prelude::{just, one_of},
    Parser as _,
};

use super::csv::cell;

pub fn date() -> impl chumsky::Parser<char, NaiveDate, Error = Simple<char>> {
    let digit = || one_of("0123456789");
    let separator = just('-');
    let year = digit().repeated().exactly(4).try_map(parse_number::<i32>);
    let month_or_day = || digit().repeated().exactly(2).try_map(parse_number::<u32>);
    year.then_ignore(separator)
        .then(month_or_day())
        .then_ignore(separator)
        .then(month_or_day())
        .try_map(|((year, month), day), span| {
            NaiveDate::from_ymd_opt(year, month, day)
                .ok_or_else(|| Simple::custom(span, "Invalid date"))
        })
        .labelled("date")
}

fn parse_number<N: FromStr>(content: Vec<char>, span: Range<usize>) -> Result<N, Simple<char>> {
    content
        .into_iter()
        .collect::<String>()
        .parse()
        .map_err(|_err| Simple::custom(span, "Failed to parse number"))
}

pub fn date_range() -> impl chumsky::Parser<char, (NaiveDate, NaiveDate), Error = Simple<char>> {
    date()
        .then_ignore(just(" to "))
        .then(date())
        .labelled("date range")
}

pub fn date_cell() -> impl chumsky::Parser<char, NaiveDate, Error = Simple<char>> {
    cell(date()).labelled("date cell")
}

#[cfg(test)]
mod tests {
    use chumsky::Error as _;

    use crate::import::parser::utils::testutils::test_parser;

    use super::*;

    #[test]
    fn test_date() {
        test_parser(
            "2021-01-01",
            date(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            "",
        );
        test_parser(
            "2021-01-31",
            date(),
            NaiveDate::from_ymd_opt(2021, 1, 31).unwrap(),
            "",
        );
        test_parser(
            "2021-02-28",
            date(),
            NaiveDate::from_ymd_opt(2021, 2, 28).unwrap(),
            "",
        );

        assert_eq!(
            date().parse("2021-02-29"),
            Err(vec![
                Simple::custom(0..10, "Invalid date").with_label("date")
            ])
        );
        test_parser(
            "2021-12-31",
            date(),
            NaiveDate::from_ymd_opt(2021, 12, 31).unwrap(),
            "",
        );
        test_parser(
            "1980-05-14",
            date(),
            NaiveDate::from_ymd_opt(1980, 5, 14).unwrap(),
            "",
        );
        assert_eq!(
            date().parse("1980-05-32"),
            Err(vec![
                Simple::custom(0..10, "Invalid date").with_label("date")
            ])
        );
        assert_eq!(
            date().parse("1980-13-14"),
            Err(vec![
                Simple::custom(0..10, "Invalid date").with_label("date")
            ])
        );
        assert_eq!(
            date().parse("1980-00-14"),
            Err(vec![
                Simple::custom(0..10, "Invalid date").with_label("date")
            ])
        );
        assert_eq!(
            date().parse("1980-05-00"),
            Err(vec![
                Simple::custom(0..10, "Invalid date").with_label("date")
            ])
        );
        assert_eq!(
            date().parse("1980-5-14"),
            Err(vec![Simple::expected_input_found(
                6..7,
                [
                    Some('8'),
                    Some('0'),
                    Some('2'),
                    Some('5'),
                    Some('9'),
                    Some('7'),
                    Some('3'),
                    Some('4'),
                    Some('6'),
                    Some('1'),
                ],
                Some('-')
            )
            .with_label("date")])
        );
        assert_eq!(
            date().parse("1980-05-5"),
            Err(vec![Simple::expected_input_found(
                9..9,
                [
                    Some('5'),
                    Some('1'),
                    Some('7'),
                    Some('2'),
                    Some('0'),
                    Some('4'),
                    Some('3'),
                    Some('6'),
                    Some('8'),
                    Some('9'),
                ],
                None
            )
            .with_label("date")])
        );
    }

    #[test]
    fn test_date_range() {
        test_parser(
            "2021-01-01 to 2021-01-31",
            date_range(),
            (
                NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2021, 1, 31).unwrap(),
            ),
            "",
        );
        test_parser(
            "2021-01-01 to 2021-12-31",
            date_range(),
            (
                NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2021, 12, 31).unwrap(),
            ),
            "",
        );
        test_parser(
            "2021-01-01 to 2021-12-31 ",
            date_range(),
            (
                NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2021, 12, 31).unwrap(),
            ),
            " ",
        );
        test_parser(
            "2021-01-01 to 2021-12-31\n",
            date_range(),
            (
                NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2021, 12, 31).unwrap(),
            ),
            "\n",
        );
        test_parser(
            "2021-01-01 to 2021-12-31\n ",
            date_range(),
            (
                NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2021, 12, 31).unwrap(),
            ),
            "\n ",
        );
        test_parser(
            "2021-01-01 to 2021-12-31\n\n",
            date_range(),
            (
                NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
                NaiveDate::from_ymd_opt(2021, 12, 31).unwrap(),
            ),
            "\n\n",
        );
    }

    #[test]
    fn test_date_cell() {
        test_parser(
            "2021-01-01",
            date_cell(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            "",
        );
        test_parser(
            "2021-01-01,",
            date_cell(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            ",",
        );
        test_parser(
            "2021-01-01,foo",
            date_cell(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            ",foo",
        );
        test_parser(
            "2021-01-01\nfoo",
            date_cell(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            "\nfoo",
        );
        test_parser(
            "2021-01-01\rfoo",
            date_cell(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            "\rfoo",
        );

        test_parser(
            "\"2021-01-01\"",
            date_cell(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            "",
        );
        test_parser(
            "\"2021-01-01\",",
            date_cell(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            ",",
        );
        test_parser(
            "\"2021-01-01\",foo",
            date_cell(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            ",foo",
        );
        test_parser(
            "\"2021-01-01\"\nfoo",
            date_cell(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            "\nfoo",
        );
        test_parser(
            "\"2021-01-01\"\rfoo",
            date_cell(),
            NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
            "\rfoo",
        );
    }
}
