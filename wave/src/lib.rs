use anyhow::Result;

mod args;
mod config;
mod export;
mod import;
mod ir;
mod operations;

pub fn main() -> Result<()> {
    let args = args::parse();
    let file = std::fs::File::open(args.from_csv).unwrap();

    let ledger = import::load(file).unwrap();
    let ledger = operations::merge_transactions_with_same_date_description_and_amount(ledger);
    let ledger = operations::sort_transactions_by_date(ledger);
    operations::check_transactions_are_balanced_per_date(&ledger)?;

    let config =
        config::prompt_edit_config(ledger.account_names().into_iter().map(str::to_string))?;

    export::print_exported_transactions(ledger, &config)?;

    Ok(())
}
