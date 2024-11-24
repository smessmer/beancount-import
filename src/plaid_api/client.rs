use plaid::{PlaidAuth, PlaidClient};

pub struct Plaid {
    client: PlaidClient,
}

impl Plaid {
    pub fn new(auth: PlaidAuth) -> Plaid {
        Plaid {
            client: PlaidClient::new_with(
                httpclient::Client::new().base_url("https://production.plaid.com"),
                auth,
            ),
        }
    }

    pub(super) fn client(&self) -> &PlaidClient {
        &self.client
    }
}
