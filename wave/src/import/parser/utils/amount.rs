use chumsky::{error::Simple, Parser as _};
use rust_decimal::Decimal;

use super::csv::any_cell;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Amount {
    pub amount: Decimal,
    pub currency_symbol: char,
}

pub fn amount_cell() -> impl chumsky::Parser<char, Amount, Error = Simple<char>> {
    any_cell()
        .try_map(|content, span| {
            parse_amount_content(content).map_err(|msg| Simple::custom(span, msg))
        })
        .labelled("amount cell")
}

pub fn amount_cell_opt() -> impl chumsky::Parser<char, Option<Amount>, Error = Simple<char>> {
    any_cell()
        .try_map(|content, span| {
            if content.is_empty() {
                Ok(None)
            } else {
                Ok(Some(
                    parse_amount_content(content).map_err(|msg| Simple::custom(span, msg))?,
                ))
            }
        })
        .labelled("amount cell or empty cell")
}

fn parse_amount_content(mut content: String) -> Result<Amount, &'static str> {
    let negative = if content.starts_with('-') {
        content.remove(0);
        true
    } else {
        false
    };
    if content.is_empty() {
        return Err("Expected amount, found empty string");
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
    use chumsky::Error as _;
    use rstest::rstest;

    use crate::import::parser::utils::testutils::test_parser;

    use super::*;

    #[rstest]
    fn test_amount_cell(
        #[values('$', '€')] currency_symbol: char,
        #[values(true, false)] quoted: bool,
        #[values(("123.45", Decimal::new(12345, 2)), ("0.00", Decimal::new(0, 2)))]
        (input, expected): (&str, Decimal),
    ) {
        use crate::import::parser::utils::testutils::test_parser;

        let input = if quoted {
            format!("\"{}{}\"", currency_symbol, input)
        } else {
            format!("{}{}", currency_symbol, input)
        };
        let expected = Amount {
            amount: expected,
            currency_symbol,
        };
        test_parser(&input, amount_cell(), expected, "");
        test_parser(&input, amount_cell_opt(), Some(expected), "");
    }

    #[test]
    fn without_dollar_sign() {
        assert_eq!(
            amount_cell().parse("123.45"),
            Err(vec![Simple::custom(
                0..6,
                "Expected amount to start with dollar or euro sign"
            )
            .with_label("amount cell")])
        );
        assert_eq!(
            amount_cell_opt().parse("123.45"),
            Err(vec![Simple::custom(
                0..6,
                "Expected amount to start with dollar or euro sign"
            )
            .with_label("amount cell or empty cell")])
        );
    }

    #[test]
    fn invalid_amount() {
        assert_eq!(
            amount_cell().parse("$123.4.5"),
            Err(vec![
                Simple::custom(0..8, "Failed to parse amount").with_label("amount cell")
            ])
        );
        assert_eq!(
            amount_cell_opt().parse("$123.4.5"),
            Err(vec![Simple::custom(0..8, "Failed to parse amount")
                .with_label("amount cell or empty cell")])
        );
    }

    #[test]
    fn with_space() {
        assert_eq!(
            amount_cell().parse("$123.45 "),
            Err(vec![
                Simple::custom(0..8, "Failed to parse amount").with_label("amount cell")
            ])
        );
        assert_eq!(
            amount_cell_opt().parse("$123.45 "),
            Err(vec![Simple::custom(0..8, "Failed to parse amount")
                .with_label("amount cell or empty cell")])
        );
    }

    #[test]
    fn with_thousand_separator() {
        let input = "\"$1,234.56\"";
        let expected = Amount {
            amount: Decimal::new(123456, 2),
            currency_symbol: '$',
        };
        test_parser(input, amount_cell(), expected, "");
        test_parser(input, amount_cell_opt(), Some(expected), "");
    }

    #[test]
    fn empty_cell() {
        assert_eq!(
            amount_cell().parse(""),
            Err(vec![Simple::custom(
                0..0,
                "Expected amount, found empty string"
            )
            .with_label("amount cell")])
        );
        assert!(amount_cell().parse("").is_err());
        test_parser("", amount_cell_opt(), None, "");
        test_parser(",", amount_cell_opt(), None, ",");
        test_parser("\n", amount_cell_opt(), None, "\n");
        test_parser("\r\n", amount_cell_opt(), None, "\r\n");
    }

    #[test]
    fn negative_amount() {
        let input = "\"$-123.45\"";
        let expected = Amount {
            amount: Decimal::new(-12345, 2),
            currency_symbol: '$',
        };
        test_parser(input, amount_cell(), expected, "");
        test_parser(input, amount_cell_opt(), Some(expected), "");
    }
}
