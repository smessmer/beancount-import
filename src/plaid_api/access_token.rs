use crate::db::DbAccessToken;

// TODO Remove Debug for security since the token is a secret
#[derive(Debug)]
pub struct AccessToken {
    access_token: String,
}

impl AccessToken {
    pub fn new(access_token: String) -> AccessToken {
        AccessToken { access_token }
    }

    pub(super) fn get(&self) -> &str {
        &self.access_token
    }

    pub fn to_db(&self) -> DbAccessToken {
        DbAccessToken::new(self.access_token.clone())
    }
}
