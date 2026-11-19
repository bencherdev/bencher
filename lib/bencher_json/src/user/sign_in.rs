#[cfg(feature = "plus")]
use bencher_valid::PlanLevel;
use bencher_valid::{Email, Jwt, LinkSecret, NonEmpty, SignInCode};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "plus")]
use crate::OAuthAuthorizationUuid;
use crate::{JsonUser, OrganizationUuid};

crate::typed_uuid::typed_uuid!(PendingSignInUuid);

/// Start a sign-in by email, which signs in the address's account or makes one.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonEmailSignIn {
    pub email: Email,
    /// An invitation to an organization, accepted once the address is proven.
    pub invite: Option<Jwt>,
    /// An unclaimed organization, claimed once the address is proven.
    pub claim: Option<OrganizationUuid>,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    /// An app's pending request to connect, which the sign-in returns to.
    #[cfg(feature = "plus")]
    pub authorization: Option<OAuthAuthorizationUuid>,
    /// A Cloudflare Turnstile token, where the server asks for one.
    pub turnstile: Option<NonEmpty>,
}

/// Resend the pending sign-in's email with a new code and link.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonEmailResend {
    /// A Cloudflare Turnstile token, where the server asks for one.
    pub turnstile: Option<NonEmpty>,
}

/// The address a sign-in emails: either an email went out, or the address is at its cap and none
/// did.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignInEmail {
    pub email: Email,
    /// Seconds until Resend opens, once an email went out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resend_after: Option<u32>,
    /// Seconds until the address can get another email, while it is at its cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait: Option<u32>,
}

/// Sign in with the code from the email, in the browser that asked for it.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignInCode {
    pub code: SignInCode,
}

/// Sign in with the link from the email.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignInLink {
    pub secret: LinkSecret,
    /// Sign in this browser, though it did not ask for the link.
    #[serde(default)]
    pub confirm: bool,
}

/// A finished sign-in, whose session rides in its cookie.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignedIn {
    pub user: JsonUser,
    /// Whether the sign-in made the account.
    pub new: bool,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    /// An app's pending request to connect, which the sign-in returns to.
    #[cfg(feature = "plus")]
    pub authorization: Option<OAuthAuthorizationUuid>,
}

/// What a link did: signed in, or, in a browser that did not ask for it, named the address it signs
/// in to and changed nothing, until the reader confirms.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignInAs {
    pub email: Email,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_in: Option<JsonSignedIn>,
}

/// Why a sign-in endpoint refused a request, as the `error_code` of its error.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignInRefusal {
    /// The request came from somewhere other than the console's own origin.
    CrossOrigin,
    /// The reader's address made too many attempts. The `Retry-After` header holds the seconds
    /// until it has room for another.
    TooManyAttempts,
    /// Turnstile refused the token, so the console needs a new one.
    TurnstileFailed,
    /// The invitation the sign-in carries is not valid.
    InvalidInvite,
    /// The invitation the sign-in carries is for another address.
    InviteForAnotherEmail,
    /// The organization the sign-in's invitation or claim names does not exist.
    UnknownOrganization,
    /// The email could not be sent, so no sign-in started.
    EmailFailed,
    /// The pending sign-in has expired, ended, or never was.
    SignInEnded,
    /// A wrong code, with tries left. The `Bencher-Tries-Left` header holds how many.
    WrongCode,
    /// A wrong code that used the last try, which ended the pending sign-in.
    NoTriesLeft,
    /// Too many wrong codes for the address in 24 hours, so its codes, the right one included,
    /// are refused before they are compared; its link still works. The `Retry-After` header holds
    /// the seconds until codes resume.
    CodesPaused,
    /// Resend is not open yet. The `Retry-After` header holds the seconds until it is.
    ResendWaiting,
    /// The account is locked.
    Locked,
}

impl AsRef<str> for SignInRefusal {
    fn as_ref(&self) -> &str {
        match self {
            Self::CrossOrigin => "cross_origin",
            Self::TooManyAttempts => "too_many_attempts",
            Self::TurnstileFailed => "turnstile_failed",
            Self::InvalidInvite => "invalid_invite",
            Self::InviteForAnotherEmail => "invite_for_another_email",
            Self::UnknownOrganization => "unknown_organization",
            Self::EmailFailed => "email_failed",
            Self::SignInEnded => "sign_in_ended",
            Self::WrongCode => "wrong_code",
            Self::NoTriesLeft => "no_tries_left",
            Self::CodesPaused => "codes_paused",
            Self::ResendWaiting => "resend_waiting",
            Self::Locked => "locked",
        }
    }
}

/// The header of a wrong code's refusal that holds the tries left.
pub const TRIES_LEFT: &str = "Bencher-Tries-Left";
