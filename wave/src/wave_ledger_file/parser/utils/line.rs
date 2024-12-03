use nom::{
    bytes::complete::tag,
    error::{context, VerboseError},
    sequence::terminated,
    IResult,
};

use super::csv::row_end;

/// Matches a full line with fixed content
pub fn line_tag(
    expected_line: &str,
) -> impl Fn(&str) -> IResult<&str, &str, VerboseError<&str>> + use<'_> {
    move |input| {
        context(
            "Failed to parse line_tag",
            terminated(tag(expected_line), row_end),
        )(input)
    }
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
}
