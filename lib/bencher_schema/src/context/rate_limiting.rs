use std::{
    collections::VecDeque,
    net::Ipv4Addr,
    time::{Duration, SystemTime},
};

use bencher_json::{DateTime, PlanLevel, UserUuid, system::config::JsonRateLimiting};
use bencher_license::Licensor;
use dashmap::DashMap;
use dropshot::HttpError;
use slog::Logger;

use crate::{
    error::{BencherResource, too_many_requests},
    model::{
        organization::{QueryOrganization, plan::LicenseUsage},
        project::{QueryProject, branch::QueryBranch, threshold::QueryThreshold},
        user::QueryUser,
    },
};

use super::DbConnection;

const HOUR: Duration = Duration::from_secs(60 * 60);
const DAY: Duration = Duration::from_secs(60 * 60 * 24);

const USER_LIMIT: u32 = u8::MAX as u32;
const UNCLAIMED_LIMIT: u32 = u8::MAX as u32;
const CLAIMED_LIMIT: u32 = u16::MAX as u32;

// Allow for 1 login and 3 email retries by default
const AUTH_MAX_ATTEMPTS: u32 = 4;

pub struct RateLimiting {
    pub window: Duration,
    pub user_limit: u32,
    pub unclaimed_limit: u32,
    pub claimed_limit: u32,
    pub unclaimed_runs: DashMap<Ipv4Addr, VecDeque<SystemTime>>,
    pub auth_window: Duration,
    pub auth_max_attempts: u32,
    pub auth_attempts: DashMap<UserUuid, VecDeque<SystemTime>>,
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

    #[error(
        "Too many runs from unclaimed IP address. Please, claim the project or try again later."
    )]
    UnclaimedRuns,
    #[error("Too many authentication attempts for user. Please, try again later.")]
    AuthAttempts,
}

impl From<JsonRateLimiting> for RateLimiting {
    fn from(json: JsonRateLimiting) -> Self {
        let JsonRateLimiting {
            window,
            user_limit,
            unclaimed_limit,
            claimed_limit,
            auth_window,
            auth_max_attempts,
        } = json;
        Self {
            window: window.map(u64::from).map_or(DAY, Duration::from_secs),
            user_limit: user_limit.unwrap_or(USER_LIMIT),
            unclaimed_limit: unclaimed_limit.unwrap_or(UNCLAIMED_LIMIT),
            claimed_limit: claimed_limit.unwrap_or(CLAIMED_LIMIT),
            unclaimed_runs: DashMap::new(),
            auth_window: auth_window.map(u64::from).map_or(HOUR, Duration::from_secs),
            auth_max_attempts: auth_max_attempts.unwrap_or(AUTH_MAX_ATTEMPTS),
            auth_attempts: DashMap::new(),
        }
    }
}

impl Default for RateLimiting {
    fn default() -> Self {
        Self {
            window: DAY,
            user_limit: USER_LIMIT,
            unclaimed_limit: UNCLAIMED_LIMIT,
            claimed_limit: CLAIMED_LIMIT,
            unclaimed_runs: DashMap::new(),
            auth_window: HOUR,
            auth_max_attempts: AUTH_MAX_ATTEMPTS,
            auth_attempts: DashMap::new(),
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
                    slog::warn!(
                        log,
                        "Custom rate limits are set, but there is no valid Bencher Plus license key! This is a violation of the Bencher License: https://bencher.dev/legal/license"
                    );
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

    pub fn unclaimed_run(&self, remote_ip: Ipv4Addr) -> Result<(), HttpError> {
        let now = SystemTime::now();

        // Clean up old runs for all unclaimed remote IPs
        self.unclaimed_runs.retain(|_, runs| {
            runs.retain(|&time| time > now - self.window);
            !runs.is_empty()
        });

        let mut entry = self
            .unclaimed_runs
            .entry(remote_ip)
            .or_insert_with(|| VecDeque::with_capacity(self.unclaimed_limit as usize));

        // Check if limit exceeded
        if entry.len() < self.unclaimed_limit as usize {
            // Record the new run
            entry.push_back(now);

            Ok(())
        } else {
            // Remove the oldest run and add the new one
            entry.pop_front();
            entry.push_back(now);

            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunUnclaimedMaxRuns);

            Err(too_many_requests(RateLimitingError::UnclaimedRuns))
        }
    }

    pub fn auth_attempt(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        let now = SystemTime::now();

        // Clean up old attempts for all users
        self.auth_attempts.retain(|_, attempts| {
            attempts.retain(|&time| time > now - self.auth_window);
            !attempts.is_empty()
        });

        let mut entry = self
            .auth_attempts
            .entry(user_uuid)
            .or_insert_with(|| VecDeque::with_capacity(self.auth_max_attempts as usize));

        // Check if limit exceeded
        if entry.len() < self.auth_max_attempts as usize {
            // Record the new attempt
            entry.push_back(now);

            Ok(())
        } else {
            // Remove the oldest attempt and add the new one
            entry.pop_front();
            entry.push_back(now);

            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserMaxAttempts);

            Err(too_many_requests(RateLimitingError::AuthAttempts))
        }
    }
}
