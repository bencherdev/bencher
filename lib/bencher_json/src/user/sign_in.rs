#[cfg(feature = "plus")]
use bencher_valid::PlanLevel;
use bencher_valid::{DateTime, Email, InviteSecret, LinkSecret, NonEmpty, SignInCode};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{JsonUser, OrganizationUuid};
#[cfg(feature = "plus")]
use crate::{OAuthAuthorizationUuid, SignInProvider};

crate::typed_uuid::typed_uuid!(PendingSignInUuid);

/// Start a sign-in by email, which signs in the address's account or makes one.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonEmailSignIn {
    pub email: Email,
    /// The secret of an invitation to an organization, accepted once the address is proven.
    pub invite: Option<InviteSecret>,
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
///
/// A fresh sign-in's link proves again the session that asked for it.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSignInAs {
    pub email: Email,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_in: Option<JsonSignedIn>,
    /// When a fresh sign-in's link proved the session again.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proved: Option<DateTime>,
}

/// How the signed-in reader can sign in again: by an email to their address, always, and with
/// each provider pinned to them that the server signs in with.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonFreshSignIn {
    #[cfg(feature = "plus")]
    pub providers: Vec<SignInProvider>,
}

/// A fresh sign-in, which proved the session again and changed nothing else.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProved {
    pub proved: DateTime,
}

/// Why a sign-in endpoint, or an action that needs a fresh sign-in, refused a request, as the
/// `error_code` of its error.
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
    /// A sign-in with GitHub or Google failed, for a reason it does not tell.
    ProviderRefused,
    /// The action needs a browser session whose reader proved who they are within the last 10
    /// minutes, so the console asks the reader to sign in again.
    FreshSignIn,
    /// A fresh sign-in's link, opened in a browser other than the one that asked, which it never
    /// proves.
    AskedElsewhere,
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
            Self::ProviderRefused => "provider_refused",
            Self::FreshSignIn => "fresh_sign_in",
            Self::AskedElsewhere => "asked_elsewhere",
        }
    }
}

/// The header of a wrong code's refusal that holds the tries left.
pub const TRIES_LEFT: &str = "Bencher-Tries-Left";
