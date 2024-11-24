use clap::{Parser, Subcommand};

/// Download transactions from Plaid and export them to Beancount.
#[derive(Parser, Debug)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new database file in the local directory
    Init,

    /// Add a bank connection to the database
    AddConnection,

    /// List all bank connections in the database
    ListConnections,

    /// Download transactions from plaid and put them in the local database
    Sync,

    /// Print the list of transactions in the database
    ListTransactions,
}

pub fn parse() -> Args {
    Args::parse()
}
