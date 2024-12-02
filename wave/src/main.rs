fn main() {
    // TODO clap, input file as arg
    let file = std::fs::File::open(
        "/home/heinzi/Downloads/Personal Account Transactions 2024-12-02-05_33.csv",
    )
    .unwrap();
    let wave_ledger = beancount_import_wave::wave_ledger::parse(file);
    println!("{:?}", wave_ledger);
}
