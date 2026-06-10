use std::fmt;

const AUDIENCE_AUTH: &str = "auth";
const AUDIENCE_CLIENT: &str = "client";
const AUDIENCE_API_KEY: &str = "api_key";
const AUDIENCE_INVITE: &str = "invite";
const AUDIENCE_OAUTH: &str = "oauth";
const AUDIENCE_OCI_PUBLIC: &str = "oci_public";
const AUDIENCE_OCI_AUTH: &str = "oci_auth";
const AUDIENCE_OCI_PROJECT: &str = "oci_project";
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
    OciProject,
    OciRunner,
}

impl Audience {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auth => AUDIENCE_AUTH,
            Self::Client => AUDIENCE_CLIENT,
            Self::ApiKey => AUDIENCE_API_KEY,
            Self::Invite => AUDIENCE_INVITE,
            Self::OAuth => AUDIENCE_OAUTH,
            Self::OciPublic => AUDIENCE_OCI_PUBLIC,
            Self::OciAuth => AUDIENCE_OCI_AUTH,
            Self::OciProject => AUDIENCE_OCI_PROJECT,
            Self::OciRunner => AUDIENCE_OCI_RUNNER,
        }
    }
}

impl fmt::Display for Audience {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<Audience> for String {
    fn from(audience: Audience) -> Self {
        audience.to_string()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::Audience;

    /// Every `Audience` variant paired with its expected wire string.
    const AUDIENCES: [(Audience, &str); 9] = [
        (Audience::Auth, "auth"),
        (Audience::Client, "client"),
        (Audience::ApiKey, "api_key"),
        (Audience::Invite, "invite"),
        (Audience::OAuth, "oauth"),
        (Audience::OciPublic, "oci_public"),
        (Audience::OciAuth, "oci_auth"),
        (Audience::OciProject, "oci_project"),
        (Audience::OciRunner, "oci_runner"),
    ];

    #[test]
    fn audience_as_str_matches_expected() {
        for (audience, expected) in AUDIENCES {
            assert_eq!(audience.as_str(), expected);
        }
    }

    #[test]
    fn audience_display_matches_as_str() {
        for (audience, expected) in AUDIENCES {
            assert_eq!(audience.to_string(), expected);
        }
    }

    #[test]
    fn audience_string_from_matches_display() {
        for (audience, expected) in AUDIENCES {
            assert_eq!(String::from(audience), expected);
        }
    }
}
