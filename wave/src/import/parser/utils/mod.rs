mod amount;
mod csv;
mod date;
mod line;
#[cfg(test)]
mod testutils;

pub use amount::{amount_cell, amount_cell_opt};
use chumsky::{error::Simple, Parser as _};
pub use csv::{any_cell, cell_tag, comma, empty_cell, row_end};
pub use date::{date_cell, date_range};
pub use line::{line_any_content, line_tag};
use nom::{
    error::{VerboseError, VerboseErrorKind},
    IResult,
};
#[cfg(test)]
pub use testutils::test_parser;

/// Convert a chumsky parser to a nom parser.
/// WARNING: This currently only works for parsers that always consume at least one byte, the reason is https://github.com/zesterer/chumsky/issues/707
pub fn chumsky_to_nom<T>(
    parser: impl chumsky::Parser<char, T, Error = Simple<char>>,
) -> impl Fn(&str) -> IResult<&str, T, VerboseError<&str>> {
    let parser = parser.map_with_span(|parsed, span| (parsed, span));
    move |input| {
        let (parsed, span) = parser.parse(input).map_err(|err| {
            eprintln!("Failed to parse chumsky: {:?}", err);
            nom::Err::Error(VerboseError {
                errors: vec![(input, VerboseErrorKind::Context("chumsky"))],
            })
        })?;
        let forward_by_num_bytes = input.chars().take(span.end).map(|c| c.len_utf8()).sum();
        Ok((&input[forward_by_num_bytes..], parsed))
    }
}

#[cfg(test)]
mod tests {
    use chumsky::prelude::just;

    use super::*;

    #[test]
    fn test_chumsky_to_nom() {
        let parser = just("foo");
        assert_eq!(chumsky_to_nom(parser)("foo"), Ok(("", "foo")));
        assert_eq!(chumsky_to_nom(parser)("foobar"), Ok(("bar", "foo")));
        assert_eq!(
            chumsky_to_nom(parser)("bar"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![("bar", nom::error::VerboseErrorKind::Context("chumsky"))]
            }))
        );
    }
}
