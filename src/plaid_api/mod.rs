mod accounts;
mod client;
mod link_account;
mod test_connection;
mod transactions;

pub use accounts::get_accounts;
pub use client::Plaid;
pub use link_account::link_new_account;
pub use test_connection::test_connection;
pub use transactions::get_transactions;
