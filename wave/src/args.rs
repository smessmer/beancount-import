use clap::Parser;

/// Import transactions from a Wave CSV and export to beancount
#[derive(Parser, Debug)]
pub struct Args {
    /// Path to the Wave CSV file
    #[clap(short, long)]
    pub from_csv: String,
}

pub fn parse() -> Args {
    Args::parse()
}
