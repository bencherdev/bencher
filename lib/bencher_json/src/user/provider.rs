use bencher_valid::{Email, Jwt, NonEmpty, PlanLevel, ProviderState, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{JsonSignedIn, OAuthAuthorizationUuid, OrganizationUuid, SessionMethod};

/// A provider a reader signs in with.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum SignInProvider {
    Github = GITHUB_INT,
    Google = GOOGLE_INT,
}

const GITHUB_INT: i32 = 0;
const GOOGLE_INT: i32 = 1;

impl SignInProvider {
    /// The provider's name, as a reader knows it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Github => "GitHub",
            Self::Google => "Google",
        }
    }
}

impl From<SignInProvider> for SessionMethod {
    fn from(provider: SignInProvider) -> Self {
        match provider {
            SignInProvider::Github => Self::Github,
            SignInProvider::Google => Self::Google,
        }
    }
}

/// Start a sign-in with a provider, carrying what it applies once it finishes.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProviderStart {
    /// An invitation to an organization, accepted once an address it is for is proven.
    pub invite: Option<Jwt>,
    /// An unclaimed organization, claimed once the reader signs in.
    pub claim: Option<OrganizationUuid>,
    pub plan: Option<PlanLevel>,
    /// An app's pending request to connect, which the sign-in returns to.
    pub authorization: Option<OAuthAuthorizationUuid>,
}

/// Finish a sign-in with what the provider sent back.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProviderFinish {
    pub code: Secret,
    pub state: ProviderState,
}

/// What a provider's sign-in did: signed in, or, for a provider account pinned to no one whose
/// address has an account, emailed that address to prove it first.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProviderSignIn {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_in: Option<JsonSignedIn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inbox: Option<JsonProviderInbox>,
}

/// The address a provider account must prove before it pins: either an email went out, or the
/// address is at its cap and none did.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProviderInbox {
    /// The provider account, as its owner knows it.
    pub handle: NonEmpty,
    pub email: Email,
    /// Seconds until Resend opens, once an email went out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resend_after: Option<u32>,
    /// Seconds until the address can get another email, while it is at its cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait: Option<u32>,
}

#[cfg(feature = "db")]
mod sign_in_provider {
    use super::{GITHUB_INT, GOOGLE_INT, SignInProvider};

    #[derive(Debug, thiserror::Error)]
    pub enum SignInProviderError {
        #[error("Invalid sign-in provider value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for SignInProvider
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Github => GITHUB_INT.to_sql(out),
                Self::Google => GOOGLE_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for SignInProvider
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                GITHUB_INT => Ok(Self::Github),
                GOOGLE_INT => Ok(Self::Google),
                value => Err(Box::new(SignInProviderError::Invalid(value))),
            }
        }
    }
}
