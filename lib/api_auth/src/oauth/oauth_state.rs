use std::sync::LazyLock;

use bencher_json::{Email, Jwt, OAuthAuthorizationUuid, OrganizationUuid, PlanLevel};
use bencher_schema::error::unauthorized_error;
use bencher_token::TokenKey;
use dropshot::HttpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use slog::Logger;

const OAUTH_TTL: u32 = 600;

#[expect(clippy::expect_used, reason = "static")]
static OAUTH_EMAIL: LazyLock<Email> =
    LazyLock::new(|| "oauth@bencher.dev".parse().expect("Invalid OAuth email"));

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct OAuthState {
    /// Invitation JWT.
    pub invite: Option<Jwt>,
    /// Organization UUID to claim.
    pub claim: Option<OrganizationUuid>,
    /// Plan level.
    pub plan: Option<PlanLevel>,
    /// An app's pending request to connect, to return to after signing in.
    pub authorization: Option<OAuthAuthorizationUuid>,
}

impl OAuthState {
    pub fn new(
        invite: Option<Jwt>,
        claim: Option<OrganizationUuid>,
        plan: Option<PlanLevel>,
    ) -> Self {
        Self {
            invite,
            claim,
            plan,
            authorization: None,
        }
    }

    pub fn encode(self, log: &Logger, token_key: &TokenKey) -> Result<Jwt, HttpError> {
        token_key
            .new_oauth(OAUTH_EMAIL.clone(), OAUTH_TTL, self.into())
            .map_err(|e| {
                let err = "Failed to create OAuth state token";
                slog::warn!(log, "{err}: {e}");
                unauthorized_error(err)
            })
    }

    pub fn decode(log: &Logger, token_key: &TokenKey, token: &Jwt) -> Result<Self, HttpError> {
        token_key
            .validate_oauth(token)
            .map(Into::into)
            .map_err(|e| {
                let err = "Failed to validate OAuth state token";
                slog::warn!(log, "{err}: {e}");
                unauthorized_error(err)
            })
    }

    pub fn invite(&self) -> Option<&Jwt> {
        self.invite.as_ref()
    }

    pub fn claim(&self) -> Option<OrganizationUuid> {
        self.claim
    }

    pub fn plan(&self) -> Option<PlanLevel> {
        self.plan
    }

    pub fn authorization(&self) -> Option<OAuthAuthorizationUuid> {
        self.authorization
    }
}

impl From<OAuthState> for bencher_token::StateClaims {
    fn from(state: OAuthState) -> Self {
        let OAuthState {
            invite,
            claim,
            plan,
            authorization,
        } = state;
        Self {
            invite,
            claim,
            plan,
            authorization,
        }
    }
}

impl From<bencher_token::OAuthClaims> for OAuthState {
    fn from(claims: bencher_token::OAuthClaims) -> Self {
        let bencher_token::StateClaims {
            invite,
            claim,
            plan,
            authorization,
        } = claims.state;
        Self {
            invite,
            claim,
            plan,
            authorization,
        }
    }
}
