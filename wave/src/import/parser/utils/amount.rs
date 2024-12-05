use nom::{
    combinator::map_res,
    error::{context, VerboseError},
    IResult,
};
use rust_decimal::Decimal;

use super::csv::cell;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Amount {
    pub amount: Decimal,
    pub currency_symbol: char,
}

pub fn amount_cell(input: &str) -> IResult<&str, Amount, VerboseError<&str>> {
    context(
        "Failed to parse amount_cell",
        map_res(cell, parse_amount_content),
    )(input)
}

pub fn amount_cell_opt(input: &str) -> IResult<&str, Option<Amount>, VerboseError<&str>> {
    context(
        "Failed to parse amount_cell_opt",
        map_res(cell, |content| {
            if content.is_empty() {
                Ok(None)
            } else {
                parse_amount_content(content).map(Some)
            }
        }),
    )(input)
}

fn parse_amount_content(mut content: String) -> Result<Amount, &'static str> {
    let negative = if content.starts_with('-') {
        content.remove(0);
        true
    } else {
        false
    };
    if content.is_empty() {
        return Err("Empty amount");
    }
    let currency_symbol = match content.remove(0) {
        '$' => '$',
        '€' => '€',
        _ => return Err("Expected amount to start with dollar or euro sign"),
    };
    let content = content.replace(',', "");
    let amount = Decimal::from_str_exact(&content).map_err(|_| "Failed to parse amount")?;
    let amount = if negative { -amount } else { amount };
    Ok(Amount {
        amount,
        currency_symbol,
    })
}

#[cfg(test)]
mod test {
    use rstest::rstest;

    use super::*;

    #[rstest]
    fn test_amount_cell(
        #[values('$', '€')] currency_symbol: char,
        #[values(true, false)] quoted: bool,
        #[values(("123.45", Decimal::new(12345, 2)), ("0.00", Decimal::new(0, 2)))]
        (input, expected): (&str, Decimal),
    ) {
        let input = if quoted {
            format!("\"{}{}\"", currency_symbol, input)
        } else {
            format!("{}{}", currency_symbol, input)
        };
        let expected = Amount {
            amount: expected,
            currency_symbol,
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
            Ok((
                "",
                Amount {
                    currency_symbol: '$',
                    amount: Decimal::new(123456, 2),
                }
            ))
        );
        assert_eq!(
            amount_cell_opt("\"$1,234.56\""),
            Ok((
                "",
                Some(Amount {
                    currency_symbol: '$',
                    amount: Decimal::new(123456, 2),
                })
            ))
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
            Ok((
                "",
                Amount {
                    currency_symbol: '$',
                    amount: Decimal::new(-12345, 2)
                }
            ))
        );
        assert_eq!(
            amount_cell_opt("\"$-123.45\""),
            Ok((
                "",
                Some(Amount {
                    currency_symbol: '$',
                    amount: Decimal::new(-12345, 2)
                })
            ))
        );
    }
}
