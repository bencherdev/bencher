use std::fmt;

const AUDIENCE_AUTH: &str = "auth";
const AUDIENCE_CLIENT: &str = "client";
const AUDIENCE_API_KEY: &str = "api_key";
const AUDIENCE_INVITE: &str = "invite";
const AUDIENCE_OAUTH: &str = "oauth";

#[derive(Debug, Copy, Clone)]
pub enum Audience {
    Auth,
    Client,
    ApiKey,
    Invite,
    OAuth,
}
impl fmt::Display for Audience {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Auth => AUDIENCE_AUTH,
                Self::Client => AUDIENCE_CLIENT,
                Self::ApiKey => AUDIENCE_API_KEY,
                Self::Invite => AUDIENCE_INVITE,
                Self::OAuth => AUDIENCE_OAUTH,
            }
        )
    }
}

impl From<Audience> for String {
    fn from(audience: Audience) -> Self {
        audience.to_string()
    }
}
