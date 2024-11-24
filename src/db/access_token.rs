use serde::{Deserialize, Serialize};

// TODO Overwrite Debug for security since the token is a secret
#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct AccessToken {
    access_token: String,
}

impl AccessToken {
    pub fn new(access_token: String) -> AccessToken {
        AccessToken { access_token }
    }

    pub fn get(&self) -> &str {
        &self.access_token
    }
}
