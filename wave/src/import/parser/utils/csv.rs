use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_till},
    character::complete::line_ending,
    combinator::{eof, map_res, value},
    error::{context, VerboseError},
    multi::many0,
    sequence::delimited,
    IResult, Parser,
};

/// Match a CSV cell, either enclosed in quotes or unquoted. The commas around the cell are not matched.
pub fn cell(input: &str) -> IResult<&str, String, VerboseError<&str>> {
    context("Failed to parse cell", alt((quoted_cell, unquoted_cell)))(input)
}

fn quoted_cell(input: &str) -> IResult<&str, String, VerboseError<&str>> {
    delimited(tag("\""), quoted_cell_content, tag("\""))(input)
}

fn quoted_cell_content(input: &str) -> IResult<&str, String, VerboseError<&str>> {
    let quoted_quote = value("\"", tag("\"\""));
    many0(alt((quoted_quote, is_not("\""))))
        .map(|chars| chars.into_iter().collect::<String>())
        .parse(input)
}

fn unquoted_cell(input: &str) -> IResult<&str, String, VerboseError<&str>> {
    take_till(|c| c == ',' || c == '\n' || c == '\r')
        .map(str::to_string)
        .parse(input)
}

/// Match an empty cell
pub fn empty_cell(input: &str) -> IResult<&str, (), VerboseError<&str>> {
    context("Failed to parse empty_cell", value((), cell_tag("")))(input)
}

/// Match a cell with specific content
pub fn cell_tag(
    expected_content: &str,
) -> impl Fn(&str) -> IResult<&str, String, VerboseError<&str>> + use<'_> {
    move |input| {
        context(
            "Failed to parse cell_tag",
            map_res(cell, move |actual_content| {
                if actual_content == expected_content {
                    Ok(actual_content)
                } else {
                    Err("Unexpected content")
                }
            }),
        )(input)
    }
}

pub fn comma(input: &str) -> IResult<&str, (), VerboseError<&str>> {
    context("Failed to parse comma", value((), tag(",")))(input)
}

pub fn row_end(input: &str) -> IResult<&str, (), VerboseError<&str>> {
    context(
        "Failed to parse row_end",
        value((), alt((line_ending, eof))),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quoted_cell() {
        assert_eq!(quoted_cell("\"foo\""), Ok(("", "foo".to_string())));
        assert_eq!(quoted_cell("\"foo,bar\""), Ok(("", "foo,bar".to_string())));
        assert_eq!(
            quoted_cell("\"foo,bar\nbaz\""),
            Ok(("", "foo,bar\nbaz".to_string()))
        );
        assert_eq!(
            quoted_cell("\"foo,bar\rbaz\""),
            Ok(("", "foo,bar\rbaz".to_string()))
        );
        assert_eq!(
            quoted_cell("\"foo,bar\"baz"),
            Ok(("baz", "foo,bar".to_string()))
        );
        assert_eq!(
            quoted_cell("\"foo,\"\",bar\"baz"),
            Ok(("baz", "foo,\",bar".to_string()))
        );
    }

    #[test]
    fn test_unquoted_cell() {
        assert_eq!(unquoted_cell("foo"), Ok(("", "foo".to_string())));
        assert_eq!(unquoted_cell("foo,"), Ok((",", "foo".to_string())));
        assert_eq!(unquoted_cell("foo,bar"), Ok((",bar", "foo".to_string())));
        assert_eq!(unquoted_cell("foo\nbar"), Ok(("\nbar", "foo".to_string())));
        assert_eq!(unquoted_cell("foo\rbar"), Ok(("\rbar", "foo".to_string())));
    }

    #[test]
    fn test_cell() {
        assert_eq!(cell("foo"), Ok(("", "foo".to_string())));
        assert_eq!(cell("foo,"), Ok((",", "foo".to_string())));
        assert_eq!(cell("foo,bar"), Ok((",bar", "foo".to_string())));
        assert_eq!(cell("\"foo\""), Ok(("", "foo".to_string())));
        assert_eq!(cell("\"foo\",bar"), Ok((",bar", "foo".to_string())));
        assert_eq!(cell("\"foo,bar\"baz"), Ok(("baz", "foo,bar".to_string())));
        assert_eq!(
            cell("\"foo,bar\nbaz\""),
            Ok(("", "foo,bar\nbaz".to_string()))
        );
        assert_eq!(cell("foo\nbar"), Ok(("\nbar", "foo".to_string())));
        assert_eq!(
            cell("\"foo,bar\rbaz\""),
            Ok(("", "foo,bar\rbaz".to_string()))
        );
        assert_eq!(cell("foo\rbar"), Ok(("\rbar", "foo".to_string())));
        assert_eq!(
            cell("\"foo,\"\",bar\"baz"),
            Ok(("baz", "foo,\",bar".to_string()))
        );
    }

    #[test]
    fn test_empty_cell() {
        assert_eq!(empty_cell(""), Ok(("", ())));
        assert_eq!(empty_cell(","), Ok((",", ())));
        assert_eq!(empty_cell(",bla"), Ok((",bla", ())));
        assert_eq!(empty_cell("\"\""), Ok(("", ())));
        assert_eq!(empty_cell("\"\","), Ok((",", ())));
        assert_eq!(empty_cell("\"\",bla"), Ok((",bla", ())));
        assert_eq!(
            empty_cell("foo"),
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
                    )
                ]
            }))
        );
    }

    #[test]
    fn test_cell_tag() {
        assert_eq!(cell_tag("foo")("foo"), Ok(("", "foo".to_string())));
        assert_eq!(cell_tag("foo")("foo,"), Ok((",", "foo".to_string())));
        assert_eq!(cell_tag("foo")("foo,bar"), Ok((",bar", "foo".to_string())));
        assert_eq!(cell_tag("foo")("\"foo\""), Ok(("", "foo".to_string())));
        assert_eq!(
            cell_tag("foo")("\"foo\",bar"),
            Ok((",bar", "foo".to_string()))
        );
        assert_eq!(
            cell_tag("foo")("\"foo,bar\"baz"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "\"foo,bar\"baz",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "\"foo,bar\"baz",
                        nom::error::VerboseErrorKind::Context("Failed to parse cell_tag")
                    )
                ]
            }))
        );
        assert_eq!(
            cell_tag("foo")("\"foo,bar\nbaz\""),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "\"foo,bar\nbaz\"",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "\"foo,bar\nbaz\"",
                        nom::error::VerboseErrorKind::Context("Failed to parse cell_tag")
                    )
                ]
            }))
        );
        assert_eq!(
            cell_tag("foo")("foo\nbar"),
            Ok(("\nbar", "foo".to_string()))
        );
        assert_eq!(
            cell_tag("foo")("\"foo,bar\rbaz\""),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "\"foo,bar\rbaz\"",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "\"foo,bar\rbaz\"",
                        nom::error::VerboseErrorKind::Context("Failed to parse cell_tag")
                    )
                ]
            }))
        );
        assert_eq!(
            cell_tag("foo")("foo\rbar"),
            Ok(("\rbar", "foo".to_string()))
        );
        assert_eq!(
            cell_tag("foo")("\"foo,\"\",bar\"baz"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "\"foo,\"\",bar\"baz",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "\"foo,\"\",bar\"baz",
                        nom::error::VerboseErrorKind::Context("Failed to parse cell_tag")
                    )
                ]
            }))
        );
        assert_eq!(
            cell_tag("foo")("bar"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "bar",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::MapRes)
                    ),
                    (
                        "bar",
                        nom::error::VerboseErrorKind::Context("Failed to parse cell_tag")
                    )
                ]
            }))
        );
    }

    #[test]
    fn test_comma() {
        assert_eq!(comma(","), Ok(("", ())));
        assert_eq!(
            comma("foo"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "foo",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::Tag)
                    ),
                    (
                        "foo",
                        nom::error::VerboseErrorKind::Context("Failed to parse comma")
                    )
                ]
            }))
        );
    }

    #[test]
    fn test_row_end() {
        assert_eq!(row_end("\nbla"), Ok(("bla", ())));
        assert_eq!(row_end("\r\nbla"), Ok(("bla", ())));
        assert_eq!(row_end(""), Ok(("", ())));
        assert_eq!(
            row_end("foo"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "foo",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::Eof)
                    ),
                    (
                        "foo",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::Alt)
                    ),
                    (
                        "foo",
                        nom::error::VerboseErrorKind::Context("Failed to parse row_end")
                    )
                ]
            }))
        );
    }
}
