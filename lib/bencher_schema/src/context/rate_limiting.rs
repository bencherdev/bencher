use std::{
    collections::VecDeque,
    hash::Hash,
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
    window: Duration,
    user_limit: u32,
    unclaimed_limit: u32,
    claimed_limit: u32,
    unclaimed_runs: DashMap<Ipv4Addr, VecDeque<SystemTime>>,
    auth_window: Duration,
    auth_max_attempts: u32,
    auth_attempts: DashMap<UserUuid, VecDeque<SystemTime>>,
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
    #[error("Unclaimed organization ({uuid}) has exceeded the daily rate limit ({rate_limit}). Please, reduce your daily usage or claim the organization: https://bencher.dev/auth/signup?claim={uuid}", uuid = organization.uuid)]
    UnclaimedOrganization {
        organization: QueryOrganization,
        rate_limit: u32,
    },
    #[error("No plan (subscription or license) found for claimed organization ({uuid}) that exceeds the daily rate limit ({rate_limit}). Please, reduce your daily usage or purchase a Bencher Plus plan: https://bencher.dev/pricing", uuid = organization.uuid)]
    ClaimedOrganization {
        organization: QueryOrganization,
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

    pub fn check_user_limit<F>(&self, window_usage: u32, error_fn: F) -> Result<(), HttpError>
    where
        F: Fn(u32) -> RateLimitingError,
    {
        Self::check_inner(self.user_limit, window_usage, error_fn)
    }

    pub fn check_claimable_limit<FUn, FCl>(
        &self,
        is_claimed: bool,
        window_usage: u32,
        unclaimed_error_fn: FUn,
        claimed_error_fn: FCl,
    ) -> Result<(), HttpError>
    where
        FUn: Fn(u32) -> RateLimitingError,
        FCl: Fn(u32) -> RateLimitingError,
    {
        if is_claimed {
            Self::check_inner(self.claimed_limit, window_usage, claimed_error_fn)
        } else {
            Self::check_inner(self.unclaimed_limit, window_usage, unclaimed_error_fn)
        }
    }

    pub fn check_unclaimed_limit<F>(&self, window_usage: u32, error_fn: F) -> Result<(), HttpError>
    where
        F: Fn(u32) -> RateLimitingError,
    {
        Self::check_inner(self.unclaimed_limit, window_usage, error_fn)
    }

    pub fn check_claimed_limit<F>(&self, window_usage: u32, error_fn: F) -> Result<(), HttpError>
    where
        F: Fn(u32) -> RateLimitingError,
    {
        Self::check_inner(self.claimed_limit, window_usage, error_fn)
    }

    fn check_inner<F>(limit: u32, window_usage: u32, error_fn: F) -> Result<(), HttpError>
    where
        F: Fn(u32) -> RateLimitingError,
    {
        if window_usage < limit {
            Ok(())
        } else {
            Err(too_many_requests(error_fn(limit)))
        }
    }

    pub fn unclaimed_run(&self, remote_ip: Ipv4Addr) -> Result<(), HttpError> {
        check_rate_limit(
            &self.unclaimed_runs,
            remote_ip,
            self.window,
            self.unclaimed_limit as usize,
            #[cfg(feature = "otel")]
            bencher_otel::ApiCounter::RunUnclaimedMaxRuns,
            RateLimitingError::UnclaimedRuns,
        )
    }

    pub fn auth_attempt(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        check_rate_limit(
            &self.auth_attempts,
            user_uuid,
            self.auth_window,
            self.auth_max_attempts as usize,
            #[cfg(feature = "otel")]
            bencher_otel::ApiCounter::UserMaxAttempts,
            RateLimitingError::AuthAttempts,
        )
    }
}

fn check_rate_limit<K>(
    dash_map: &DashMap<K, VecDeque<SystemTime>>,
    key: K,
    window: Duration,
    limit: usize,
    #[cfg(feature = "otel")] api_counter: bencher_otel::ApiCounter,
    error: RateLimitingError,
) -> Result<(), HttpError>
where
    K: PartialEq + Eq + Hash,
{
    let now = SystemTime::now();

    // Clean up old times for all keys
    dash_map.retain(|_, times| {
        times.retain(|&time| time > now - window);
        !times.is_empty()
    });

    let mut entry = dash_map
        .entry(key)
        .or_insert_with(|| VecDeque::with_capacity(limit));

    // Check if the limit has been exceeded
    if entry.len() < limit {
        // Record the new time for the key
        entry.push_back(now);

        Ok(())
    } else {
        // Remove the oldest time and add the new one
        entry.pop_front();
        entry.push_back(now);

        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(api_counter);

        Err(too_many_requests(error))
    }
}
