use std::fmt;

const AUDIENCE_AUTH: &str = "auth";
const AUDIENCE_CLIENT: &str = "client";
const AUDIENCE_API_KEY: &str = "api_key";
const AUDIENCE_INVITE: &str = "invite";
const AUDIENCE_OAUTH: &str = "oauth";
const AUDIENCE_OCI_PUBLIC: &str = "oci_public";
const AUDIENCE_OCI_AUTH: &str = "oci_auth";
const AUDIENCE_OCI_RUNNER: &str = "oci_runner";

#[derive(Debug, Copy, Clone)]
pub enum Audience {
    Auth,
    Client,
    ApiKey,
    Invite,
    OAuth,
    OciPublic,
    OciAuth,
    OciRunner,
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
                Self::OciPublic => AUDIENCE_OCI_PUBLIC,
                Self::OciAuth => AUDIENCE_OCI_AUTH,
                Self::OciRunner => AUDIENCE_OCI_RUNNER,
            }
        )
    }
}

impl From<Audience> for String {
    fn from(audience: Audience) -> Self {
        audience.to_string()
    }
}
