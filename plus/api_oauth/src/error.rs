use bencher_json::UserUuid;
use bencher_schema::{
    context::{ApiContext, OAuthServer, RateLimiting},
    error::issue_error,
    model::user::oauth::OAuthGrantFailure,
};
use dropshot::{Body, HttpError, RequestContext};
use http::{Response, StatusCode};
use slog::Logger;

use crate::response::oauth_error;

/// Why the token or revocation endpoint refused a request (RFC 6749, section 5.2).
/// Every description is fixed text that never quotes the request.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TokenError {
    Request(InvalidRequest),
    Client,
    GrantType,
    Grant(GrantRefusal),
    Scope,
    Target,
    Throttled,
    Server,
}

impl TokenError {
    pub(crate) fn server(error: diesel::result::Error) -> Self {
        let _err = issue_error(
            "Failed to answer an OAuth token request",
            "Failed to answer an OAuth token request:",
            error,
        );
        Self::Server
    }

    /// The pool reports its own failures, so this one is not reported again.
    pub(crate) fn pool(_reported: HttpError) -> Self {
        Self::Server
    }

    /// The refusal, charged to the caller's address when the request named no known code or
    /// token, which makes it anonymous; over the address's limit, it is refused for that instead.
    /// Any other refusal is its user's, and its answer stands.
    pub(crate) fn charged(self, rqctx: &RequestContext<ApiContext>) -> Self {
        if self.is_anonymous() {
            charge_address(rqctx).err().unwrap_or(self)
        } else {
            self
        }
    }

    fn is_anonymous(self) -> bool {
        matches!(
            self,
            Self::Request(_)
                | Self::Client
                | Self::GrantType
                | Self::Target
                | Self::Grant(GrantRefusal::UnknownCode | GrantRefusal::UnknownToken)
        )
    }

    pub(crate) fn into_response(
        self,
        log: &Logger,
        oauth: &OAuthServer,
    ) -> Result<Response<Body>, HttpError> {
        slog::info!(log, "OAuth token request refused";
            "error" => self.code(),
            "reason" => ?self,
        );
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthTokenFailed(self.into()));
        oauth_error(
            self.status(),
            self.code(),
            self.description(),
            oauth.issuer(),
        )
    }

    fn status(self) -> StatusCode {
        match self {
            Self::Request(_) | Self::GrantType | Self::Grant(_) | Self::Scope | Self::Target => {
                StatusCode::BAD_REQUEST
            },
            Self::Client => StatusCode::UNAUTHORIZED,
            Self::Throttled => StatusCode::TOO_MANY_REQUESTS,
            Self::Server => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::Request(_) => "invalid_request",
            Self::Client => "invalid_client",
            Self::GrantType => "unsupported_grant_type",
            Self::Grant(_) => "invalid_grant",
            Self::Scope => "invalid_scope",
            Self::Target => "invalid_target",
            Self::Throttled => "temporarily_unavailable",
            Self::Server => "server_error",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Request(invalid) => invalid.description(),
            Self::Client => {
                "Only public clients are supported: send client_id, and no client credentials"
            },
            Self::GrantType => {
                "Only the authorization_code and refresh_token grant types are supported"
            },
            Self::Grant(_) => {
                "The authorization code or refresh token is invalid, expired, revoked, or was issued to another client"
            },
            Self::Scope => "The scope asked for is more than the user granted",
            Self::Target => "The resource is not one this server issues tokens for",
            Self::Throttled => "Too many requests, please try again later",
            Self::Server => "The request could not be completed, please try again",
        }
    }
}

/// Charge a request for a known code or token to its user. Even past the user's limit, the request
/// is answered for what it is, since a dead credential's answer tells its client to sign in again.
pub(crate) fn charge_user(context: &ApiContext, user_uuid: UserUuid) {
    let _over_limit = context.rate_limiting.user_request(user_uuid).is_err();
}

/// Charge an anonymous request to its address, refusing it once the address is over its limit.
pub(crate) fn charge_address(rqctx: &RequestContext<ApiContext>) -> Result<(), TokenError> {
    if let Some(remote_ip) = RateLimiting::remote_ip(&rqctx.log, rqctx.request.headers()) {
        rqctx
            .context()
            .rate_limiting
            .public_request(remote_ip)
            .map_err(|_err| TokenError::Throttled)?;
    }
    Ok(())
}

/// What is wrong with a malformed request.
#[derive(Debug, Clone, Copy)]
pub(crate) enum InvalidRequest {
    Repeated,
    GrantType,
    CodeParameters,
    CodeVerifier,
    RefreshParameters,
    RevokeParameters,
}

impl InvalidRequest {
    fn description(self) -> &'static str {
        match self {
            Self::Repeated => "A request parameter is repeated",
            Self::GrantType => "The grant_type parameter is required",
            Self::CodeParameters => {
                "The code, client_id, and code_verifier parameters are required"
            },
            Self::CodeVerifier => {
                "The code_verifier parameter must be 43 to 128 unreserved characters"
            },
            Self::RefreshParameters => "The refresh_token and client_id parameters are required",
            Self::RevokeParameters => "The token and client_id parameters are required",
        }
    }
}

/// Why an authorization code or refresh token was refused, for logs and telemetry only:
/// the client always hears the same `invalid_grant`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum GrantRefusal {
    UnknownCode,
    CodeReused,
    CodeExpired,
    OtherClient,
    RedirectUri,
    Resource,
    CodeVerifier,
    ResourceUnserved,
    UnknownToken,
    /// Known when read, gone by the time it was written.
    Vanished,
    TokenKind,
    TokenReused,
    TokenExpired,
    Grant(OAuthGrantFailure),
}

#[cfg(feature = "otel")]
impl From<TokenError> for bencher_otel::OAuthTokenFailureReason {
    fn from(error: TokenError) -> Self {
        match error {
            TokenError::Request(_) => Self::InvalidRequest,
            TokenError::Client => Self::InvalidClient,
            TokenError::GrantType => Self::UnsupportedGrantType,
            TokenError::Grant(refusal) => refusal.into(),
            TokenError::Scope => Self::InvalidScope,
            TokenError::Target => Self::InvalidTarget,
            TokenError::Throttled => Self::Throttled,
            TokenError::Server => Self::ServerError,
        }
    }
}

#[cfg(feature = "otel")]
impl From<GrantRefusal> for bencher_otel::OAuthTokenFailureReason {
    fn from(refusal: GrantRefusal) -> Self {
        match refusal {
            GrantRefusal::UnknownCode => Self::UnknownCode,
            GrantRefusal::CodeReused => Self::CodeReused,
            GrantRefusal::CodeExpired => Self::CodeExpired,
            GrantRefusal::OtherClient => Self::OtherClient,
            GrantRefusal::RedirectUri => Self::RedirectUri,
            GrantRefusal::Resource => Self::Resource,
            GrantRefusal::CodeVerifier => Self::CodeVerifier,
            GrantRefusal::ResourceUnserved => Self::ResourceUnserved,
            GrantRefusal::UnknownToken => Self::UnknownToken,
            GrantRefusal::Vanished => Self::Vanished,
            GrantRefusal::TokenKind => Self::TokenKind,
            GrantRefusal::TokenReused => Self::TokenReused,
            GrantRefusal::TokenExpired => Self::TokenExpired,
            GrantRefusal::Grant(OAuthGrantFailure::Revoked) => Self::GrantRevoked,
            GrantRefusal::Grant(OAuthGrantFailure::Expired) => Self::GrantExpired,
            GrantRefusal::Grant(OAuthGrantFailure::Locked) => Self::Locked,
            GrantRefusal::Grant(OAuthGrantFailure::KeyRevoked) => Self::KeyRevoked,
            GrantRefusal::Grant(OAuthGrantFailure::KeyExpired) => Self::KeyExpired,
        }
    }
}
