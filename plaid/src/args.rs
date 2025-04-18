use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Download transactions from Plaid and export them to Beancount.
#[derive(Parser, Debug)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Command,

    /// Path to the database file
    #[clap(long)]
    pub db_path: PathBuf,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new database file in the local directory
    Init,

    /// Add a bank connection to the database
    AddConnection,

    /// List all bank connections in the database
    ListConnections,

    /// Remove a bank connection from the database
    RemoveConnection {
        #[clap(short, long)]
        connection_name: String,
    },

    /// Download transactions from plaid and put them in the local database
    Sync,

    /// Print the list of transactions in the database
    ListTransactions,

    /// Export all transactions from the database to a Beancount file
    ExportAll,

    /// Export new transactions from the database to a Beancount file,
    /// and mark those transactions as exported so future calls to this
    /// command will not include them.
    ExportNew,
}

pub fn parse() -> Args {
    Args::parse()
}
