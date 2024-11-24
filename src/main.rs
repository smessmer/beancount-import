use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = beancount_plaid::args::parse();
    beancount_plaid::cli::main(args).await
}
