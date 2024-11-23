use plaid::{PlaidAuth, PlaidClient};

pub struct Plaid {
    client: PlaidClient,
}

impl Plaid {
    pub fn new() -> Plaid {
        Plaid {
            client: PlaidClient::with_auth(PlaidAuth::from_env()),
        }
    }

    pub(super) fn client(&self) -> &PlaidClient {
        &self.client
    }
}
