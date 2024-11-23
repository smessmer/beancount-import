use std::net::{IpAddr, Ipv4Addr};

use anyhow::Result;
use rocket::{get, response::content::RawHtml, routes, Config, Shutdown, State};
use std::sync::Mutex;

use crate::plaid_api::{LinkToken, PublicToken};

const LISTEN_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
const LISTEN_PORT: u16 = 8080;

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
    .mount("/", routes![show_auth_page, submit_token_api])
    .ignite()
    .await?;

    open::that(format!("http://{LISTEN_ADDR}:{LISTEN_PORT}"))?;

    // start server and wait for it to shutdown
    let server = server.launch().await?;
    let public_token = server
        .state::<ServerState>()
        .unwrap()
        .public_token
        .lock()
        .unwrap()
        .take()
        .unwrap();
    Ok(public_token)
}

#[get("/")]
fn show_auth_page(state: &State<ServerState>) -> RawHtml<String> {
    let link_token = &state.link_token.link_token;
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
    *state.public_token.lock().unwrap() = Some(PublicToken {
        public_token: token.to_string(),
    });
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
