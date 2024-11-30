use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = beancount_import_plaid::args::parse();
    beancount_import_plaid::cli::main(args).await
}
