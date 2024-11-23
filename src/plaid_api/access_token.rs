// TODO Remove Debug for security since the token is a secret
#[derive(Debug)]
pub struct AccessToken {
    access_token: String,
}

impl AccessToken {
    pub fn new(access_token: String) -> AccessToken {
        AccessToken { access_token }
    }
}
