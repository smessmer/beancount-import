use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter};

#[derive(Serialize, Deserialize, Clone)]
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

impl Debug for AccessToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "AccessToken(*****)")
    }
}
