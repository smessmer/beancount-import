use std::net::{IpAddr, Ipv4Addr};

use anyhow::Result;
use console::style;
use rocket::{get, http::ContentType, response::content::RawHtml, routes, Config, Shutdown, State};
use std::sync::Mutex;

use super::tokens::{LinkToken, PublicToken};

const LISTEN_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
const LISTEN_PORT: u16 = 8080;

const FAVICON_ICO: &[u8] = include_bytes!("static/logo.ico");

struct ServerState {
    link_token: LinkToken,
    public_token: Mutex<Option<PublicToken>>,
}

pub async fn link_in_browser(link_token: LinkToken) -> Result<PublicToken> {
    let server = rocket::custom(Config {
        log_level: rocket::config::LogLevel::Critical,
        address: LISTEN_ADDR,
        port: LISTEN_PORT,
        ..Default::default()
    })
    .manage(ServerState {
        link_token: link_token,
        public_token: Mutex::new(None),
    })
    .mount("/", routes![show_auth_page, submit_token_api, favicon])
    .ignite()
    .await?;

    let url = format!("http://{LISTEN_ADDR}:{LISTEN_PORT}");

    println!("Starting in-browser link flow.");
    println!("If it doesn't open automatically, please open the following URL in your browser:");
    println!("{}", style(&url).cyan().italic());
    open::that(url)?;

    // start server and wait for it to shutdown
    let server = server.launch().await?;
    let public_token = server
        .state::<ServerState>()
        .unwrap()
        .public_token
        .lock()
        .unwrap()
        .take()
        .expect("Did not complete link flow");
    Ok(public_token)
}

#[get("/")]
fn show_auth_page(state: &State<ServerState>) -> RawHtml<String> {
    let link_token = &state.link_token.0;
    RawHtml(format!(
        r#"
        <html>
            <body>
                <script src="https://cdn.plaid.com/link/v2/stable/link-initialize.js"></script>
                <script>
                    var linkHandler = Plaid.create({{
                        token: '{link_token}',
                        onLoad: function() {{
                            // The Link module finished loading. Let's immediately open the plaid dialog.
                            console.log("onLoad");
                            linkHandler.open();
                        }},
                        onSuccess: function(public_token, metadata) {{
                            console.log("onSuccess");
                            console.log('public_token: '+public_token+', metadata: '+JSON.stringify(metadata));
                            window.location.replace("/submit_token/" + public_token);
                        }},
                        onExit: function(err, metadata) {{
                            console.log("onExit");
                            // The user exited the Link flow.
                            if (err != null) {{
                                // The user encountered a Plaid API error prior to exiting.
                            }}
                            // metadata contains information about the institution
                            // that the user selected and the most recent API request IDs.
                            // Storing this information can be helpful for support.
                        }}
                    }});
                </script>
            </body>
        </html>
    "#
    ))
}

#[get("/submit_token/<token>")]
fn submit_token_api(
    token: &str,
    state: &State<ServerState>,
    shutdown: Shutdown,
) -> RawHtml<&'static str> {
    *state.public_token.lock().unwrap() = Some(PublicToken(token.to_string()));
    shutdown.notify();
    RawHtml(
        r#"
        <html>
            <body>
                <h1>Success</h1>
                <p>You can close this page now</p>
            </body>
        </html>
    "#,
    )
}

#[get("/favicon.ico")]
fn favicon() -> (ContentType, &'static [u8]) {
    (ContentType::Icon, FAVICON_ICO)
}
