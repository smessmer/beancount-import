use nom::{
    bytes::complete::tag,
    character::complete::not_line_ending,
    error::{context, VerboseError},
    sequence::terminated,
    IResult,
};

use super::{chumsky_to_nom, csv::row_end};

/// Matches a full line with fixed content
pub fn line_tag(
    expected_line: &str,
) -> impl Fn(&str) -> IResult<&str, &str, VerboseError<&str>> + use<'_> {
    move |input| {
        context(
            "Failed to parse line_tag",
            terminated(tag(expected_line), chumsky_to_nom(row_end())),
        )(input)
    }
}

/// Matches a full line with any content
pub fn line_any_content(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
    context(
        "Failed to parse line_any_content",
        terminated(not_line_ending, chumsky_to_nom(row_end())),
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_tag() {
        assert_eq!(line_tag("foo")("foo\n"), Ok(("", "foo")));
        assert_eq!(line_tag("foo")("foo\r\n"), Ok(("", "foo")));
        assert_eq!(line_tag("foo")("foo"), Ok(("", "foo")));
        assert_eq!(
            line_tag("foo")("bar\n"),
            Err(nom::Err::Error(nom::error::VerboseError {
                errors: vec![
                    (
                        "bar\n",
                        nom::error::VerboseErrorKind::Nom(nom::error::ErrorKind::Tag)
                    ),
                    (
                        "bar\n",
                        nom::error::VerboseErrorKind::Context("Failed to parse line_tag")
                    )
                ]
            }))
        );
    }

    #[test]
    fn test_line_any_content() {
        assert_eq!(line_any_content("foo\n"), Ok(("", "foo")));
        assert_eq!(line_any_content("foo\r\n"), Ok(("", "foo")));
        assert_eq!(line_any_content("foo"), Ok(("", "foo")));
        assert_eq!(line_any_content("foo\nbar"), Ok(("bar", "foo")));
    }
}
