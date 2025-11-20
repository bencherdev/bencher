use core::fmt;

use opentelemetry::metrics::Meter;

pub struct ApiMeter {
    meter: Meter,
}

impl ApiMeter {
    const NAME: &str = "bencher_api";

    fn new() -> Self {
        let meter = opentelemetry::global::meter(Self::NAME);
        ApiMeter { meter }
    }

    pub fn increment(api_counter: ApiCounter) {
        let counter = Self::new()
            .meter
            .u64_counter(api_counter.name().to_owned())
            .with_description(api_counter.description().to_owned())
            .build();
        let attributes = api_counter.attributes();
        counter.add(1, &attributes);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ApiCounter {
    ServerStartup,
    UserSignup(AuthMethod),
    UserLogin(AuthMethod),
    UserAccept(Option<AuthMethod>),
    UserConfirm,
    UserClaim,
}

impl ApiCounter {
    fn name(&self) -> &str {
        match self {
            Self::ServerStartup => "server.startup",
            Self::UserSignup(_) => "user.signup",
            Self::UserLogin(_) => "user.login",
            Self::UserAccept(_) => "user.accept",
            Self::UserConfirm => "user.confirm",
            Self::UserClaim => "user.claim",
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::ServerStartup => "Counts the number of server startups",
            Self::UserSignup(_) => "Counts the number of user signups",
            Self::UserLogin(_) => "Counts the number of user logins",
            Self::UserAccept(_) => "Counts the number of user acceptances",
            Self::UserConfirm => "Counts the number of user confirmations",
            Self::UserClaim => "Counts the number of user claims",
        }
    }

    fn attributes(self) -> Vec<opentelemetry::KeyValue> {
        match self {
            Self::ServerStartup | Self::UserClaim => Vec::new(),
            Self::UserSignup(auth_method) | Self::UserLogin(auth_method) => {
                auth_method.attributes()
            },
            Self::UserAccept(auth_method) => AuthMethod::nullable_attributes(auth_method),
            Self::UserConfirm => AuthMethod::Email.attributes(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AuthMethod {
    Email,
    OAuth(OAuthProvider),
}

impl fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Email => write!(f, "email"),
            Self::OAuth(_) => write!(f, "oauth"),
        }
    }
}

impl From<AuthMethod> for opentelemetry::KeyValue {
    fn from(auth_method: AuthMethod) -> Self {
        opentelemetry::KeyValue::new(AuthMethod::KEY, auth_method.to_string())
    }
}

impl AuthMethod {
    const KEY: &str = "auth.method";

    fn attributes(self) -> Vec<opentelemetry::KeyValue> {
        std::iter::once(self.into())
            .chain(self.provider_attribute())
            .collect()
    }

    fn nullable_attributes(auth_method: Option<Self>) -> Vec<opentelemetry::KeyValue> {
        match auth_method {
            Some(auth_method) => auth_method.attributes(),
            None => vec![opentelemetry::KeyValue::new(AuthMethod::KEY, "unknown")],
        }
    }

    fn provider_attribute(self) -> Option<opentelemetry::KeyValue> {
        match self {
            Self::Email => None,
            Self::OAuth(provider) => Some(provider.into()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OAuthProvider {
    GitHub,
    Google,
}

impl fmt::Display for OAuthProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitHub => write!(f, "github"),
            Self::Google => write!(f, "google"),
        }
    }
}

impl From<OAuthProvider> for opentelemetry::KeyValue {
    fn from(provider: OAuthProvider) -> Self {
        opentelemetry::KeyValue::new(OAuthProvider::KEY, provider.to_string())
    }
}

impl OAuthProvider {
    const KEY: &str = "auth.provider";
}
