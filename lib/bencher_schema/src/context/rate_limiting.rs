use std::time::Duration;

use bencher_json::{system::config::JsonRateLimiting, DateTime, PlanLevel};
use bencher_license::Licensor;
use slog::Logger;

use crate::{
    error::BencherResource,
    model::{
        organization::{plan::LicenseUsage, QueryOrganization},
        project::{branch::QueryBranch, threshold::QueryThreshold, QueryProject},
        user::QueryUser,
    },
};

use super::DbConnection;

const DAY: Duration = Duration::from_secs(24 * 60 * 60);
const UNCLAIMED_RATE_LIMIT: u32 = u8::MAX as u32;
const CLAIMED_RATE_LIMIT: u32 = u16::MAX as u32;

pub struct RateLimiting {
    pub window: Duration,
    pub unclaimed_limit: u32,
    pub claimed_limit: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum RateLimitingError {
    #[error("User ({uuid}) has exceeded the daily rate limit ({rate_limit}) for {resource} creation. Please, reduce your daily usage.", uuid = user.uuid)]
    User {
        user: QueryUser,
        resource: BencherResource,
        rate_limit: u32,
    },
    #[error("Organization ({uuid}) has exceeded the daily rate limit ({rate_limit}) for {resource} creation. Please, reduce your daily usage.", uuid = organization.uuid)]
    Organization {
        organization: QueryOrganization,
        resource: BencherResource,
        rate_limit: u32,
    },
    #[error("Unclaimed project ({uuid}) has exceeded the daily rate limit ({rate_limit}) for {resource} creation. Please, reduce your daily usage or claim the project: https://bencher.dev/auth/signup?claim={uuid}", uuid = project.uuid)]
    UnclaimedProject {
        project: QueryProject,
        resource: BencherResource,
        rate_limit: u32,
    },
    #[error("Claimed project ({uuid}) has exceeded the daily rate limit ({rate_limit}) for {resource} creation. Please, reduce your daily usage.", uuid = project.uuid)]
    ClaimedProject {
        project: QueryProject,
        resource: BencherResource,
        rate_limit: u32,
    },
    #[error("Branch ({uuid}) has exceeded the daily rate limit ({rate_limit}) for {resource} creation. Please, reduce your daily usage.", uuid = branch.uuid)]
    Branch {
        branch: QueryBranch,
        resource: BencherResource,
        rate_limit: u32,
    },
    #[error("Threshold ({uuid}) has exceeded the daily rate limit ({rate_limit}) for {resource} creation. Please, reduce your daily usage.", uuid = threshold.uuid)]
    Threshold {
        threshold: QueryThreshold,
        resource: BencherResource,
        rate_limit: u32,
    },
}

impl From<JsonRateLimiting> for RateLimiting {
    fn from(json: JsonRateLimiting) -> Self {
        let JsonRateLimiting {
            window,
            unclaimed_limit,
            claimed_limit,
        } = json;
        Self {
            window: window.map(u64::from).map_or(DAY, Duration::from_secs),
            unclaimed_limit: unclaimed_limit.unwrap_or(UNCLAIMED_RATE_LIMIT),
            claimed_limit: claimed_limit.unwrap_or(CLAIMED_RATE_LIMIT),
        }
    }
}

impl Default for RateLimiting {
    fn default() -> Self {
        Self {
            window: DAY,
            unclaimed_limit: UNCLAIMED_RATE_LIMIT,
            claimed_limit: CLAIMED_RATE_LIMIT,
        }
    }
}

impl RateLimiting {
    pub async fn new(
        log: &Logger,
        conn: &tokio::sync::Mutex<DbConnection>,
        licensor: &Licensor,
        is_bencher_cloud: bool,
        rate_limiting: Option<JsonRateLimiting>,
    ) -> Result<Self, RateLimitingError> {
        let Some(rate_limiting) = rate_limiting else {
            return Ok(Self::default());
        };

        if !is_bencher_cloud {
            match LicenseUsage::get_for_server(conn, licensor, Some(PlanLevel::Team)).await {
                Ok(license_usages) if license_usages.is_empty() => {
                    slog::warn!(log, "Custom rate limits are set, but there is no valid Bencher Plus license key! This is a violation of the Bencher License: https://bencher.dev/legal/license");
                    slog::warn!(
                        log,
                        "Please purchase a license key: https://bencher.dev/pricing"
                    );
                },
                Ok(_) => {},
                Err(e) => {
                    slog::error!(log, "Failed to check license for custom rate limits: {e}");
                },
            }
        }

        Ok(rate_limiting.into())
    }

    pub fn window(&self) -> (DateTime, DateTime) {
        let end_time = chrono::Utc::now();
        let start_time = end_time - self.window;
        (start_time.into(), end_time.into())
    }
}
