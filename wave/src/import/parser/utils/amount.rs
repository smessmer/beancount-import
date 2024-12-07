use chumsky::{
    error::Simple,
    prelude::{just, one_of},
    Parser as _,
};
use rust_decimal::Decimal;

use super::csv::cell;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Amount {
    pub amount: Decimal,
    pub currency_symbol: String,
}

pub fn amount_cell() -> impl chumsky::Parser<char, Amount, Error = Simple<char>> {
    cell(amount()).labelled("amount cell")
}

pub fn amount_cell_opt() -> impl chumsky::Parser<char, Option<Amount>, Error = Simple<char>> {
    cell(amount().or_not()).labelled("amount cell or empty cell")
}

fn amount() -> impl chumsky::Parser<char, Amount, Error = Simple<char>> {
    let maybe_negative = just("-").or_not();
    let currency_symbol = just("$")
        .or(just("€"))
        .or(just("£"))
        .or(just("CHF"))
        .labelled("currency symbol");
    let amount = one_of("0123456789.")
        .then_ignore(just(',').or_not())
        .repeated()
        .at_least(1)
        .try_map(|content, span| {
            Decimal::from_str_exact(&content.into_iter().collect::<String>())
                .map_err(|_| Simple::custom(span, "Failed to parse amount"))
        })
        .labelled("number");
    maybe_negative
        .then(currency_symbol)
        .then(amount)
        .map(|((negative, currency_symbol), amount)| Amount {
            amount: if negative.is_some() { -amount } else { amount },
            currency_symbol: currency_symbol.to_string(),
        })
        .labelled("amount")
}

#[cfg(test)]
mod test {
    use chumsky::Error as _;
    use rstest::rstest;

    use crate::import::parser::utils::testutils::test_parser;

    use super::*;

    #[rstest]
    fn test_amount_cell(
        #[values("$", "€", "£", "CHF")] currency_symbol: &str,
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
            currency_symbol: currency_symbol.to_string(),
        };
        test_parser(&input, amount_cell(), expected.clone(), "");
        test_parser(&input, amount_cell_opt(), Some(expected), "");
    }

    #[test]
    fn without_dollar_sign() {
        assert_eq!(
            amount_cell().parse("123.45"),
            Err(vec![
                Simple::expected_input_found(
                    0..1,
                    [Some('£'), Some('$'), Some('C'), Some('-'), Some('€')],
                    Some('1')
                )
                .with_label("currency symbol"),
                Simple::custom(0..6, "Failed to parse cell content").with_label("csv cell")
            ])
        );
        assert_eq!(
            amount_cell_opt().parse("123.45"),
            Err(vec![
                Simple::expected_input_found(
                    0..1,
                    [Some('-'), Some('C'), Some('£'), Some('$'), Some('€')],
                    Some('1')
                )
                .with_label("currency symbol"),
                Simple::custom(0..6, "Failed to parse cell content").with_label("csv cell")
            ])
        );
    }

    #[test]
    fn invalid_amount() {
        assert_eq!(
            amount_cell().parse("$123.4.5"),
            Err(vec![
                Simple::custom(1..8, "Failed to parse amount").with_label("number"),
                Simple::custom(0..8, "Failed to parse cell content").with_label("csv cell")
            ])
        );
        assert_eq!(
            amount_cell_opt().parse("$123.4.5"),
            Err(vec![
                Simple::custom(1..8, "Failed to parse amount").with_label("number"),
                Simple::custom(0..8, "Failed to parse cell content").with_label("csv cell")
            ])
        );
    }

    #[test]
    fn with_space() {
        assert_eq!(
            amount_cell().parse("$123.45 "),
            Err(vec![
                Simple::expected_input_found(
                    7..8,
                    [
                        Some('1'),
                        None,
                        Some(','),
                        Some('.'),
                        Some('3'),
                        Some('8'),
                        Some('5'),
                        Some('4'),
                        Some('0'),
                        Some('6'),
                        Some('2'),
                        Some('7'),
                        Some('9')
                    ],
                    Some(' ')
                )
                .with_label("number"),
                Simple::custom(0..8, "Failed to parse cell content").with_label("csv cell")
            ])
        );
        assert_eq!(
            amount_cell_opt().parse("$123.45 "),
            Err(vec![
                Simple::expected_input_found(
                    7..8,
                    [
                        Some('1'),
                        None,
                        Some(','),
                        Some('.'),
                        Some('3'),
                        Some('8'),
                        Some('5'),
                        Some('4'),
                        Some('0'),
                        Some('6'),
                        Some('2'),
                        Some('7'),
                        Some('9')
                    ],
                    Some(' ')
                )
                .with_label("number"),
                Simple::custom(0..8, "Failed to parse cell content").with_label("csv cell")
            ])
        );
    }

    #[test]
    fn with_thousand_separator() {
        let input = "\"$1,234.56\"";
        let expected = Amount {
            amount: Decimal::new(123456, 2),
            currency_symbol: "$".to_string(),
        };
        test_parser(input, amount_cell(), expected.clone(), "");
        test_parser(input, amount_cell_opt(), Some(expected), "");
    }

    #[test]
    fn empty_cell() {
        assert_eq!(
            amount_cell().parse(""),
            Err(vec![
                Simple::expected_input_found(
                    0..0,
                    [Some('€'), Some('£'), Some('-'), Some('C'), Some('$')],
                    None
                )
                .with_label("currency symbol"),
                Simple::custom(0..0, "Failed to parse cell content").with_label("csv cell")
            ])
        );
        assert!(amount_cell().parse("").is_err());
        test_parser("", amount_cell_opt(), None, "");
        test_parser(",", amount_cell_opt(), None, ",");
        test_parser("\n", amount_cell_opt(), None, "\n");
        test_parser("\r\n", amount_cell_opt(), None, "\r\n");
    }

    #[test]
    fn negative_amount() {
        let input = "\"-$123.45\"";
        let expected = Amount {
            amount: Decimal::new(-12345, 2),
            currency_symbol: "$".to_string(),
        };
        test_parser(input, amount_cell(), expected.clone(), "");
        test_parser(input, amount_cell_opt(), Some(expected), "");
    }
}
