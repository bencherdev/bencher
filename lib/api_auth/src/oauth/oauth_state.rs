use base64::{Engine as _, prelude::BASE64_URL_SAFE_NO_PAD};
use bencher_json::{Jwt, OrganizationUuid, PlanLevel, Secret};
use bencher_schema::error::unauthorized_error;
use dropshot::HttpError;
use serde::{Deserialize, Serialize};
use slog::Logger;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthState {
    csrf: Uuid,
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
            csrf: Uuid::new_v4(),
            invite,
            claim,
            plan,
        }
    }

    pub fn encode(&self, log: &Logger) -> Result<Secret, HttpError> {
        serde_json::to_vec(self)
            .map(|bytes| BASE64_URL_SAFE_NO_PAD.encode(bytes))
            .inspect_err(|e| slog::warn!(log, "Failed to serialize OAuth state: {e}"))
            .map_err(unauthorized_error)?
            .parse()
            .map_err(unauthorized_error)
    }

    pub fn decode(log: &Logger, state: &Secret) -> Result<Self, HttpError> {
        BASE64_URL_SAFE_NO_PAD
            .decode(state.as_ref())
            .inspect_err(|e| slog::warn!(log, "Failed to base64 decode OAuth state: {e}"))
            .map_err(unauthorized_error)
            .and_then(|bytes| {
                serde_json::from_slice(&bytes)
                    .inspect_err(|e| {
                        slog::warn!(log, "Failed to JSON decode Google OAuth state: {e}");
                    })
                    .map_err(unauthorized_error)
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
