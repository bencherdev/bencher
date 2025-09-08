use std::sync::LazyLock;

use bencher_json::{Email, Jwt, OrganizationUuid, PlanLevel};
use bencher_schema::error::unauthorized_error;
use bencher_token::TokenKey;
use dropshot::HttpError;
use serde::{Deserialize, Serialize};
use slog::Logger;

const OAUTH_TTL: u32 = 600;

#[expect(clippy::expect_used, reason = "static")]
static OAUTH_EMAIL: LazyLock<Email> =
    LazyLock::new(|| "oauth@bencher.dev".parse().expect("Invalid OAuth email"));

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthState {
    invite: Option<Jwt>,
    claim: Option<OrganizationUuid>,
    plan: Option<PlanLevel>,
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
}

impl From<OAuthState> for bencher_token::StateClaims {
    fn from(state: OAuthState) -> Self {
        let OAuthState {
            invite,
            claim,
            plan,
        } = state;
        Self {
            invite,
            claim,
            plan,
        }
    }
}

impl From<bencher_token::OAuthClaims> for OAuthState {
    fn from(claims: bencher_token::OAuthClaims) -> Self {
        let bencher_token::StateClaims {
            invite,
            claim,
            plan,
        } = claims.state;
        Self {
            invite,
            claim,
            plan,
        }
    }
}
