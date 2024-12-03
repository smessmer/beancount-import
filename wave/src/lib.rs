use anyhow::Result;

mod export;
mod import;
mod ir;
mod operations;

pub fn main() -> Result<()> {
    // TODO clap, input file as arg
    let file = std::fs::File::open(
        "/home/heinzi/Downloads/Personal Account Transactions 2024-12-02-06_40.csv",
    )
    .unwrap();

    let ledger = import::load(file).unwrap();
    let ledger = operations::merge_transactions_with_same_date_description_and_amount(ledger);
    let ledger = operations::sort_transactions_by_date(ledger);
    operations::check_transactions_are_balanced_per_date(&ledger)?;

    export::print_exported_transactions(ledger)?;

    Ok(())
}
