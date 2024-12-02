use anyhow::Result;
use nom::{error::VerboseError, Finish, Parser};
use parser::Ledger;
use std::io::Read;

mod parser;

pub fn parse(mut input_stream: impl Read) -> Result<Ledger> {
    let mut content = String::new();
    input_stream.read_to_string(&mut content)?;
    let content = maybe_remove_byte_order_mark(content);
    let (rest, parsed) = parser::ledger
        .parse(&content)
        .finish()
        .map_err(|err| VerboseError {
            errors: err
                .errors
                .into_iter()
                .map(|(input, kind)| (input.to_string(), kind))
                .collect(),
        })?;
    assert_eq!("", rest);
    Ok(parsed)
}

fn maybe_remove_byte_order_mark(mut content: String) -> String {
    if content.starts_with("\u{FEFF}") {
        content.remove(0);
    }
    content
}
