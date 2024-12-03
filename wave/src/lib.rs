use anyhow::Result;

mod ir;
mod operations;
mod wave_ledger_file;

pub fn main() -> Result<()> {
    // TODO clap, input file as arg
    let file = std::fs::File::open(
        "/home/heinzi/Downloads/Personal Account Transactions 2024-12-02-06_40.csv",
    )
    .unwrap();

    let ledger = wave_ledger_file::load(file).unwrap();
    let ledger = operations::merge_transactions_with_same_date_and_same_description(ledger);
    operations::check_transactions_are_balanced_per_date(&ledger)?;

    println!("{:?}", ledger);

    Ok(())
}
