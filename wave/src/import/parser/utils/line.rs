use chumsky::{error::Simple, prelude::just, Parser as _};

use super::csv::row_end;

/// Matches a full line with fixed content
pub fn line_tag(
    expected_line: &str,
) -> impl chumsky::Parser<char, (), Error = Simple<char>> + use<'_> {
    just(expected_line)
        .ignore_then(row_end())
        .labelled("line with specific content")
}

/// Matches a full line with fixed content
pub fn line_any_content() -> impl chumsky::Parser<char, String, Error = Simple<char>> {
    row_end()
        .not()
        .repeated()
        .then_ignore(row_end())
        .collect()
        .labelled("line")
}

#[cfg(test)]
mod tests {
    use chumsky::Error;

    use crate::import::parser::utils::testutils::test_parser;

    use super::*;

    #[test]
    fn test_line_tag() {
        test_parser("foo\n", line_tag("foo"), (), "");
        test_parser("foo\r\n", line_tag("foo"), (), "");
        test_parser("foo", line_tag("foo"), (), "");
        test_parser("foo\nbar", line_tag("foo"), (), "bar");
        test_parser("foo\r\nbar", line_tag("foo"), (), "bar");

        assert_eq!(
            line_tag("foo").parse("bar\n"),
            Err(vec![Simple::expected_input_found(
                0..1,
                [Some('f')],
                Some('b')
            )
            .with_label("line with specific content")])
        );
    }

    #[test]
    fn test_line_any_content() {
        test_parser("foo\n", line_any_content(), "foo".to_string(), "");
        test_parser("foo\r\n", line_any_content(), "foo".to_string(), "");
        test_parser("foo", line_any_content(), "foo".to_string(), "");
        test_parser("foo\nbar", line_any_content(), "foo".to_string(), "bar");
    }
}
