use std::ops::Range;

use chumsky::{
    error::Simple,
    prelude::{any, end, just, one_of},
    Parser as _,
};

/// Match a CSV cell, either enclosed in quotes or unquoted. The commas around the cell are not matched.
pub fn cell<T>(
    content_parser: impl chumsky::Parser<char, T, Error = Simple<char>>,
) -> impl chumsky::Parser<char, T, Error = Simple<char>> {
    let content_parser = content_parser.then_ignore(end());
    quoted_cell()
        .or(unquoted_cell())
        .then_ignore(cell_end().rewind())
        .validate(
            // Take any errors thrown by the inner parser, adjust their span, and emit them.
            move |content, outer_span, emit| match content_parser.parse(content.as_str()) {
                Ok(parsed) => Ok(parsed),
                Err(inner_errors) => {
                    for err in inner_errors.into_iter() {
                        emit(err.map_span(|inner_span| Range {
                            start: outer_span.start + inner_span.start,
                            end: outer_span.start + inner_span.end,
                        }));
                    }
                    Err(Simple::custom(outer_span, "Failed to parse cell content"))
                }
            },
        )
        .try_map(|parsed, _span| parsed)
        .labelled("csv cell")
}

fn quoted_cell() -> impl chumsky::Parser<char, String, Error = Simple<char>> {
    let escaped_quote = just("\"\"").to('\"');
    let quoted_cell_content = quote().not().or(escaped_quote).repeated().collect();

    quote()
        .ignore_then(quoted_cell_content)
        .then_ignore(quote())
        .labelled("quoted csv cell")
}

fn unquoted_cell() -> impl chumsky::Parser<char, String, Error = Simple<char>> {
    let empty_unquoted_cell = cell_end()
        .rewind()
        .to(String::new())
        .labelled("empty unquoted cell");
    let nonempty_unquoted_cell = quote()
        .or(cell_end())
        .not()
        .chain(cell_end().not().repeated())
        .collect()
        .labelled("nonempty unquoted cell");

    nonempty_unquoted_cell
        .or(empty_unquoted_cell)
        .labelled("unquoted csv cell")
}

pub fn cell_end() -> impl chumsky::Parser<char, (), Error = Simple<char>> {
    one_of(",\r\n").ignored().or(end()).labelled("cell end")
}

fn quote() -> impl chumsky::Parser<char, (), Error = Simple<char>> {
    just('\"').ignored().labelled("quote")
}

/// Match a cell with any content
pub fn any_cell() -> impl chumsky::Parser<char, String, Error = Simple<char>> {
    cell(any().repeated().collect())
}

/// Match an empty cell
pub fn empty_cell() -> impl chumsky::Parser<char, (), Error = Simple<char>> {
    cell_tag("").labelled("empty cell")
}

/// Match a cell with specific content
pub fn cell_tag<'a>(
    expected_content: &'a str,
) -> impl chumsky::Parser<char, (), Error = Simple<char>> + use<'a> {
    cell(just(expected_content)).ignored()
}

pub fn comma() -> impl chumsky::Parser<char, (), Error = Simple<char>> {
    just(',').ignored().labelled("comma")
}

pub fn row_end() -> impl chumsky::Parser<char, (), Error = Simple<char>> {
    just("\r\n")
        .ignored()
        .or(just('\n').ignored())
        .or(end())
        .labelled("row end")
}

#[cfg(test)]
mod tests {
    use chumsky::Error as _;

    use crate::import::parser::utils::testutils::test_parser;

    use super::*;

    #[test]
    fn test_quoted_cell() {
        test_parser("\"foo\"", quoted_cell(), "foo".to_string(), "");
        test_parser("\"foo,bar\"", quoted_cell(), "foo,bar".to_string(), "");
        test_parser(
            "\"foo,bar\nbaz\"",
            quoted_cell(),
            "foo,bar\nbaz".to_string(),
            "",
        );
        test_parser(
            "\"foo,bar\rbaz\"",
            quoted_cell(),
            "foo,bar\rbaz".to_string(),
            "",
        );
        test_parser(
            "\"foo,bar\"baz",
            quoted_cell(),
            "foo,bar".to_string(),
            "baz",
        );
        test_parser(
            "\"foo,\"\",bar\"baz",
            quoted_cell(),
            "foo,\",bar".to_string(),
            "baz",
        );
    }

    #[test]
    fn test_unquoted_cell() {
        test_parser("", unquoted_cell(), "".to_string(), "");
        test_parser(",", unquoted_cell(), "".to_string(), ",");
        test_parser("\n", unquoted_cell(), "".to_string(), "\n");
        test_parser("\r\n", unquoted_cell(), "".to_string(), "\r\n");
        test_parser("foo", unquoted_cell(), "foo".to_string(), "");
        test_parser("foo,", unquoted_cell(), "foo".to_string(), ",");
        test_parser("foo,bar", unquoted_cell(), "foo".to_string(), ",bar");
        test_parser("foo\nbar", unquoted_cell(), "foo".to_string(), "\nbar");
        test_parser("foo\rbar", unquoted_cell(), "foo".to_string(), "\rbar");
        test_parser(
            "foo,bar\nbaz",
            unquoted_cell(),
            "foo".to_string(),
            ",bar\nbaz",
        );
        test_parser(",bar", unquoted_cell(), "".to_string(), ",bar");
        assert_eq!(
            vec![Simple::expected_input_found(
                0..1,
                [Some('\n'), Some(','), Some('\r'), None],
                Some('\"')
            )
            .with_label("unquoted csv cell")],
            unquoted_cell().parse("\"foo\"").unwrap_err(),
        );
    }

    #[test]
    fn test_any_cell() {
        test_parser("", any_cell(), "".to_string(), "");
        test_parser(",", any_cell(), "".to_string(), ",");
        test_parser("\r", any_cell(), "".to_string(), "\r");
        test_parser("\r\n", any_cell(), "".to_string(), "\r\n");
        test_parser("\"\"", any_cell(), "".to_string(), "");
        test_parser("\"\",", any_cell(), "".to_string(), ",");
        test_parser("\"\"\r", any_cell(), "".to_string(), "\r");
        test_parser("\"\"\r\n", any_cell(), "".to_string(), "\r\n");
        test_parser("foo", any_cell(), "foo".to_string(), "");
        test_parser("foo,", any_cell(), "foo".to_string(), ",");
        test_parser("foo\r", any_cell(), "foo".to_string(), "\r");
        test_parser("foo\r\n", any_cell(), "foo".to_string(), "\r\n");
        test_parser("\"foo\"", any_cell(), "foo".to_string(), "");
        test_parser("\"foo\",", any_cell(), "foo".to_string(), ",");
        test_parser("\"foo\"\r", any_cell(), "foo".to_string(), "\r");
        test_parser("\"foo\"\r\n", any_cell(), "foo".to_string(), "\r\n");
        test_parser("foo,bar", any_cell(), "foo".to_string(), ",bar");
        test_parser("foo\nbar", any_cell(), "foo".to_string(), "\nbar");
        test_parser("foo\rbar", any_cell(), "foo".to_string(), "\rbar");
        test_parser("\"foo,bar\"", any_cell(), "foo,bar".to_string(), "");
        test_parser("\"foo,bar\",", any_cell(), "foo,bar".to_string(), ",");
        test_parser("\"foo,bar\"\r", any_cell(), "foo,bar".to_string(), "\r");
        test_parser("\"foo,bar\"\r\n", any_cell(), "foo,bar".to_string(), "\r\n");
        test_parser("\"foo\",bar", any_cell(), "foo".to_string(), ",bar");
        test_parser("\"foo,bar\",baz", any_cell(), "foo,bar".to_string(), ",baz");
        test_parser(
            "\"foo,bar\"\nbaz",
            any_cell(),
            "foo,bar".to_string(),
            "\nbaz",
        );
        test_parser(
            "\"foo,bar\nbaz\"",
            any_cell(),
            "foo,bar\nbaz".to_string(),
            "",
        );
        test_parser(
            "\"foo,bar\rbaz\"",
            any_cell(),
            "foo,bar\rbaz".to_string(),
            "",
        );
        test_parser(
            "\"foo,\"\",bar\",baz",
            any_cell(),
            "foo,\",bar".to_string(),
            ",baz",
        );
        test_parser(
            "\"foo,\"\",bar\"\nbaz",
            any_cell(),
            "foo,\",bar".to_string(),
            "\nbaz",
        );
        test_parser(
            "\"foo,\"\",bar\"\r\nbaz",
            any_cell(),
            "foo,\",bar".to_string(),
            "\r\nbaz",
        );
    }

    #[test]
    fn test_empty_cell() {
        test_parser("", empty_cell(), (), "");
        test_parser(",", empty_cell(), (), ",");
        test_parser("\r", empty_cell(), (), "\r");
        test_parser("\r\n", empty_cell(), (), "\r\n");
        test_parser("\"\"", empty_cell(), (), "");
        test_parser("\"\",", empty_cell(), (), ",");
        test_parser("\"\"\r", empty_cell(), (), "\r");
        test_parser("\"\"\r\n", empty_cell(), (), "\r\n");
        test_parser(",foo", empty_cell(), (), ",foo");
        test_parser("\rfoo", empty_cell(), (), "\rfoo");
        test_parser("\r\nfoo", empty_cell(), (), "\r\nfoo");
        test_parser("\"\",foo", empty_cell(), (), ",foo");
        test_parser("\"\"\rfoo", empty_cell(), (), "\rfoo");
        test_parser("\"\"\r\nfoo", empty_cell(), (), "\r\nfoo");

        assert_eq!(
            empty_cell().parse("foo").unwrap_err(),
            vec![
                Simple::expected_input_found(0..1, [None], Some('f')).with_label("csv cell"),
                Simple::custom(0..3, "Failed to parse cell content").with_label("csv cell")
            ],
        );
    }

    #[test]
    fn test_cell_tag() {
        test_parser("", cell_tag(""), (), "");
        test_parser(",", cell_tag(""), (), ",");
        test_parser("\r", cell_tag(""), (), "\r");
        test_parser("\r\n", cell_tag(""), (), "\r\n");
        test_parser("\"\"", cell_tag(""), (), "");
        test_parser("\"\",", cell_tag(""), (), ",");
        test_parser("\"\"\r", cell_tag(""), (), "\r");
        test_parser("\"\"\r\n", cell_tag(""), (), "\r\n");
        test_parser("foo", cell_tag("foo"), (), "");
        test_parser("foo,", cell_tag("foo"), (), ",");
        test_parser("foo,bar", cell_tag("foo"), (), ",bar");
        test_parser("foo\rbar", cell_tag("foo"), (), "\rbar");
        test_parser("foo\nbar", cell_tag("foo"), (), "\nbar");
        test_parser("foo\r\nbar", cell_tag("foo"), (), "\r\nbar");
        test_parser("\"foo\"", cell_tag("foo"), (), "");
        test_parser("\"foo\",", cell_tag("foo"), (), ",");
        test_parser("\"foo,bar\"", cell_tag("foo,bar"), (), "");
        test_parser("\"foo,bar\",", cell_tag("foo,bar"), (), ",");
        test_parser("\"foo,bar\"\r", cell_tag("foo,bar"), (), "\r");
        test_parser("\"foo,bar\"\r\n", cell_tag("foo,bar"), (), "\r\n");
        test_parser("\"foo\",bar", cell_tag("foo"), (), ",bar");
        test_parser("\"foo,bar\",baz", cell_tag("foo,bar"), (), ",baz");
        test_parser("\"foo,bar\"\nbaz", cell_tag("foo,bar"), (), "\nbaz");
        test_parser("\"foo,bar\nbaz\"", cell_tag("foo,bar\nbaz"), (), "");
        test_parser("\"foo,bar\rbaz\"", cell_tag("foo,bar\rbaz"), (), "");
        test_parser("\"foo,\"\",bar\"", cell_tag("foo,\",bar"), (), "");
        test_parser("\"foo,\"\",bar\"\nbaz", cell_tag("foo,\",bar"), (), "\nbaz");
        test_parser(
            "\"foo,\"\",bar\"\r\nbaz",
            cell_tag("foo,\",bar"),
            (),
            "\r\nbaz",
        );

        assert_eq!(
            cell_tag("foo").parse("\"foo,bar\"baz"),
            Err(vec![Simple::expected_input_found(
                9..10,
                [Some(',')],
                Some('b')
            )
            .with_label("csv cell")])
        );
        assert_eq!(
            cell_tag("foo").parse("\"foo,bar\nbaz\""),
            Err(vec![
                Simple::expected_input_found(3..4, [None], Some(',')).with_label("csv cell"),
                Simple::custom(0..13, "Failed to parse cell content").with_label("csv cell"),
            ])
        );
        assert_eq!(
            cell_tag("foo").parse("\"foo,bar\rbaz\""),
            Err(vec![
                Simple::expected_input_found(3..4, [None], Some(',')).with_label("csv cell"),
                Simple::custom(0..13, "Failed to parse cell content").with_label("csv cell"),
            ])
        );
        assert_eq!(
            cell_tag("foo").parse("\"foo,\"\",bar\"baz"),
            Err(vec![Simple::expected_input_found(
                12..13,
                [Some('\r'), Some('\"'), Some(','), Some('\n'), None],
                Some('b')
            )
            .with_label("csv cell"),])
        );
        assert_eq!(
            cell_tag("foo").parse("bar"),
            Err(vec![
                Simple::expected_input_found(0..1, [None], Some('b')).with_label("csv cell"),
                Simple::custom(0..3, "Failed to parse cell content").with_label("csv cell"),
            ])
        );
    }

    #[test]
    fn test_comma() {
        test_parser(",", comma(), (), "");
        assert_eq!(
            comma().parse("foo").unwrap_err(),
            vec![Simple::expected_input_found(0..1, [Some(',')], Some('f')).with_label("comma")],
        );
    }

    #[test]
    fn test_row_end() {
        test_parser("\n", row_end(), (), "");
        test_parser("\r\n", row_end(), (), "");
        test_parser("\nbla", row_end(), (), "bla");
        test_parser("\r\nbla", row_end(), (), "bla");
        test_parser("", row_end(), (), "");
        assert_eq!(
            row_end().parse("foo").unwrap_err(),
            vec![Simple::expected_input_found(0..1, [None], Some('f')).with_label("row end")]
        );
    }
}
