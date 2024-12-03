use nom::{
    combinator::map_res,
    error::{context, VerboseError},
    IResult,
};
use rust_decimal::Decimal;

use super::csv::cell;

pub fn amount_cell(input: &str) -> IResult<&str, Decimal, VerboseError<&str>> {
    context("Failed to parse amount_cell", map_res(cell, parse_content))(input)
}

pub fn amount_cell_opt(input: &str) -> IResult<&str, Option<Decimal>, VerboseError<&str>> {
    context(
        "Failed to parse amount_cell_opt",
        map_res(cell, |content| {
            if content.is_empty() {
                Ok(None)
            } else {
                parse_content(content).map(Some)
            }
        }),
    )(input)
}

fn parse_content(mut content: String) -> Result<Decimal, &'static str> {
    let negative = if content.starts_with('-') {
        content.remove(0);
        true
    } else {
        false
    };
    if !content.starts_with('$') {
        return Err("Expected amount to start with dollar sign");
    }
    content.remove(0);
    let content = content.replace(',', "");
    let result = Decimal::from_str_exact(&content).map_err(|_| "Failed to parse amount")?;
    Ok(if negative { -result } else { result })
}

#[cfg(test)]
mod test {
    use rstest::rstest;

    use super::*;

    #[rstest]
    fn test_amount_cell(
        #[values(true, false)] quoted: bool,
        #[values(("123.45", Decimal::new(12345, 2)), ("0.00", Decimal::new(0, 2)))]
        (input, expected): (&str, Decimal),
    ) {
        let input = if quoted {
            format!("\"${}\"", input)
        } else {
            format!("${}", input)
        };
        assert_eq!(amount_cell(&input), Ok(("", expected)));
        assert_eq!(amount_cell_opt(&input), Ok(("", Some(expected))));
    }

    #[test]
    fn without_dollar_sign() {
        assert!(amount_cell("123.45").is_err());
        assert!(amount_cell_opt("123.45").is_err());
    }

    #[test]
    fn invalid_amount() {
        assert!(amount_cell("$123.4.5").is_err());
        assert!(amount_cell_opt("$123.4.5").is_err());
    }

    #[test]
    fn with_space() {
        assert!(amount_cell("$123.45 ").is_err());
        assert!(amount_cell_opt("$123.45 ").is_err());
    }

    #[test]
    fn with_thousand_separator() {
        assert_eq!(
            amount_cell("\"$1,234.56\""),
            Ok(("", Decimal::new(123456, 2)))
        );
        assert_eq!(
            amount_cell_opt("\"$1,234.56\""),
            Ok(("", Some(Decimal::new(123456, 2))))
        );
    }

    #[test]
    fn empty_cell() {
        assert!(amount_cell("").is_err());
        assert_eq!(amount_cell_opt(""), Ok(("", None)));
    }

    #[test]
    fn negative_amount() {
        assert_eq!(
            amount_cell("\"$-123.45\""),
            Ok(("", Decimal::new(-12345, 2)))
        );
        assert_eq!(
            amount_cell_opt("\"$-123.45\""),
            Ok(("", Some(Decimal::new(-12345, 2))))
        );
    }
}
