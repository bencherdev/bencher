const AUDIENCE_AUTH: &str = "auth";
const AUDIENCE_CLIENT: &str = "client";
const AUDIENCE_API_KEY: &str = "api_key";
const AUDIENCE_INVITE: &str = "invite";

#[derive(Debug, Copy, Clone)]
pub enum Audience {
    Auth,
    Client,
    ApiKey,
    Invite,
}

impl ToString for Audience {
    fn to_string(&self) -> String {
        match self {
            Self::Auth => AUDIENCE_AUTH.into(),
            Self::Client => AUDIENCE_CLIENT.into(),
            Self::ApiKey => AUDIENCE_API_KEY.into(),
            Self::Invite => AUDIENCE_INVITE.into(),
        }
    }
}

impl From<Audience> for String {
    fn from(audience: Audience) -> Self {
        audience.to_string()
    }
}
