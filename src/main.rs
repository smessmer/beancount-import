#[tokio::main]
async fn main() {
    let client = beancount_plaid::plaid_api::Plaid::new();
    let link_token = client.link_token_create().await.unwrap();
    println!("Link token: {}", link_token.link_token);
}
