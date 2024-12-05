use chumsky::{
    error::Simple,
    prelude::{end, just},
    Parser as _,
};

#[track_caller]
pub fn test_parser<T>(
    input: &str,
    parser: impl chumsky::Parser<char, T, Error = Simple<char>>,
    expected: T,
    rest: &str,
) where
    T: std::fmt::Debug + Eq + PartialEq,
{
    let parser = parser.then_ignore(just(rest)).then_ignore(end());
    let parsed = parser.parse(input).unwrap();
    assert_eq!(expected, parsed);
}

// TODO Use this version instead, once https://github.com/zesterer/chumsky/issues/707 is fixed
// #[track_caller]
// pub fn test_parser<T>(
//     input: &str,
//     parser: impl chumsky::Parser<char, T, Error = Simple<char>>,
//     expected: T,
//     rest: &str,
// ) where
//     T: std::fmt::Debug + Eq + PartialEq,
// {
//     let (parsed, span) = parser
//         .map_with_span(|parsed, span| (parsed, span))
//         .parse(input)
//         .unwrap();
//     assert_eq!(expected, parsed);
//     assert_eq!(rest, &input[span.end..]);
// }

#[cfg(test)]
mod tests {
    use super::*;
    use chumsky::{error::Simple, Parser};

    #[test]
    fn success_without_rest() {
        let parser = chumsky::primitive::just::<char, _, Simple<char>>('a');
        test_parser("a", parser, 'a', "");
    }

    #[test]
    fn success_with_rest() {
        let parser = chumsky::primitive::just::<char, _, Simple<char>>('a');
        test_parser("abc", parser, 'a', "bc");
    }

    #[test]
    #[should_panic(
        expected = "called `Result::unwrap()` on an `Err` value: [Simple { span: 0..1, reason: Unexpected, expected: {Some('a')}, found: Some('b'), label: None }]"
    )]
    fn parser_does_not_match() {
        let parser = chumsky::primitive::just::<char, _, Simple<char>>('a');
        test_parser("b", parser, 'a', "");
    }

    #[test]
    // #[should_panic(expected = "assertion `left == right` failed\n  left: \"bc\"\n right: \"\"")]
    #[should_panic]
    fn expected_rest_but_has_no_rest() {
        let parser = chumsky::primitive::just::<char, _, Simple<char>>('a');
        test_parser("a", parser, 'a', "bc");
    }

    #[test]
    // #[should_panic(expected = "assertion `left == right` failed\n  left: \"\"\n right: \"bc\"")]
    #[should_panic]
    fn expected_no_rest_but_has_rest() {
        let parser = chumsky::primitive::just::<char, _, Simple<char>>('a');
        test_parser("abc", parser, 'a', "");
    }

    mod regression {
        //! Regression tests for https://github.com/zesterer/chumsky/issues/707
        use super::*;

        mod empty_parser {
            use super::*;

            // TODO Add once fixed
            // #[test]
            // fn span() {
            //     let parser = chumsky::primitive::empty::<Simple<char>>().map_with_span(|(), span| span);
            //     let span = parser.parse("a");
            //     assert_eq!(span, Ok(0..0));
            // }

            #[test]
            fn success_without_rest() {
                let parser = chumsky::primitive::empty();
                test_parser("", parser, (), "");
            }

            #[test]
            fn success_with_rest() {
                let parser = chumsky::primitive::empty();
                test_parser("abc", parser, (), "abc");
            }

            #[test]
            // TODO Add expected panic message
            #[should_panic]
            fn expected_rest_but_has_no_rest() {
                let parser = chumsky::primitive::empty();
                test_parser("", parser, (), "bc");
            }

            #[test]
            // TODO Add expected panic message
            #[should_panic]
            fn expected_no_rest_but_has_rest() {
                let parser = chumsky::primitive::empty();
                test_parser("abc", parser, (), "");
            }
        }

        mod rewind_parser {
            use super::*;

            // TODO Add once fixed
            // #[test]
            // fn rewind_parser_span() {
            //     // Regression test for https://github.com/zesterer/chumsky/issues/707
            //     let parser = chumsky::primitive::just::<char, _, Simple<char>>('a')
            //         .rewind()
            //         .map_with_span(|_, span| span);
            //     let span = parser.parse("a");
            //     assert_eq!(span, Ok(0..0));
            // }

            #[test]
            fn success_without_rest() {
                let parser = chumsky::primitive::just::<char, _, Simple<char>>('a').rewind();
                test_parser("a", parser, 'a', "a");
            }

            #[test]
            fn success_with_rest() {
                let parser = chumsky::primitive::just::<char, _, Simple<char>>('a').rewind();
                test_parser("abc", parser, 'a', "abc");
            }

            #[test]
            // TDOD Add expected panic message
            #[should_panic]
            fn expected_rest_but_has_no_rest() {
                let parser = chumsky::primitive::just::<char, _, Simple<char>>('a').rewind();
                test_parser("a", parser, 'a', "abc");
            }

            #[test]
            // TDOD Add expected panic message
            #[should_panic]
            fn expected_no_rest_but_has_rest() {
                let parser = chumsky::primitive::just::<char, _, Simple<char>>('a').rewind();
                test_parser("abc", parser, 'a', "a");
            }
        }
    }
}
