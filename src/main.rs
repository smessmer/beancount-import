#[tokio::main]
async fn main() {
    let client = beancount_plaid::plaid_api::Plaid::new();
    let link_token = client.link_token_create().await.unwrap();
    println!("Link token: {}", link_token.link_token);
    let public_token = beancount_plaid::link_http_server::link_in_browser(link_token)
        .await
        .unwrap();
    println!("Public token: {}", public_token.public_token);
}
