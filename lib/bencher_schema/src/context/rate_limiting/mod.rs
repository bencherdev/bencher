use std::{net::IpAddr, time::Duration};

use bencher_json::{DateTime, PlanLevel, UserUuid, system::config::JsonRateLimiting};
use bencher_license::Licensor;
use dropshot::HttpError;
pub use http::HeaderMap;
use slog::Logger;

use crate::{
    error::{BencherResource, too_many_requests},
    model::{
        organization::{QueryOrganization, plan::LicenseUsage},
        project::{QueryProject, branch::QueryBranch, threshold::QueryThreshold},
    },
};

mod public;
mod rate_limiter;
mod remote_ip;
mod user;

use public::PublicRateLimiter;
use rate_limiter::{RateLimiter, RateLimits};
use user::UserRateLimiter;

use super::DbConnection;

const DAY: Duration = Duration::from_secs(60 * 60 * 24);

const DEFAULT_UNCLAIMED_LIMIT: u32 = u8::MAX as u32;
const DEFAULT_CLAIMED_LIMIT: u32 = u16::MAX as u32;

pub struct RateLimiting {
    // Database-backed rate limits
    window: Duration,
    unclaimed_limit: u32,
    claimed_limit: u32,
    // In-memory rate limiters
    public: PublicRateLimiter,
    user: UserRateLimiter,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RateLimitingError {
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

    #[error("Too many requests for IP address. Please, try again later.")]
    IpAddressRequests,
    #[error(
        "Too many runs from unclaimed IP address. Please, claim the project or try again later."
    )]
    UnclaimedRun,

    #[error("Too many requests for user. Please, try again later.")]
    UserRequests,
    #[error("Too many runs for user. Please, try again later.")]
    UserRuns,
    #[error("Too many authentication attempts for user. Please, try again later.")]
    UserAttempts,
    #[error("Too many token generations for user. Please, try again later.")]
    UserTokens,
    #[error("Too many organization creations for user. Please, try again later.")]
    UserOrganizations,
    #[error("Too many invitation emails for user. Please, try again later.")]
    UserInvites,
}

impl Default for RateLimiting {
    fn default() -> Self {
        Self {
            window: DAY,
            unclaimed_limit: DEFAULT_UNCLAIMED_LIMIT,
            claimed_limit: DEFAULT_CLAIMED_LIMIT,
            public: PublicRateLimiter::default(),
            user: UserRateLimiter::default(),
        }
    }
}

impl From<JsonRateLimiting> for RateLimiting {
    fn from(json: JsonRateLimiting) -> Self {
        let JsonRateLimiting {
            window,
            unclaimed_limit,
            claimed_limit,
            public,
            user,
        } = json;
        Self {
            window: window.map(u64::from).map_or(DAY, Duration::from_secs),
            unclaimed_limit: unclaimed_limit.unwrap_or(DEFAULT_UNCLAIMED_LIMIT),
            claimed_limit: claimed_limit.unwrap_or(DEFAULT_CLAIMED_LIMIT),
            public: public.map_or_else(PublicRateLimiter::default, Into::into),
            user: user.map_or_else(UserRateLimiter::default, Into::into),
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

    pub fn check_claimable_limit<FUn, FCl>(
        &self,
        is_claimed: bool,
        window_usage: u32,
        unclaimed_error_fn: FUn,
        claimed_error_fn: FCl,
    ) -> Result<(), HttpError>
    where
        FUn: FnOnce(u32) -> RateLimitingError,
        FCl: FnOnce(u32) -> RateLimitingError,
    {
        if is_claimed {
            self.check_claimed_limit(window_usage, claimed_error_fn)
        } else {
            Self::check_inner(self.unclaimed_limit, window_usage, unclaimed_error_fn)
                .inspect(|()| {
                    #[cfg(feature = "otel")]
                    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::Create(
                        bencher_otel::IntervalKind::Day,
                        bencher_otel::AuthorizationKind::Public,
                    ));
                })
                .inspect_err(|_| {
                    #[cfg(feature = "otel")]
                    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::CreateMax(
                        bencher_otel::IntervalKind::Day,
                        bencher_otel::AuthorizationKind::Public,
                    ));
                })
        }
    }

    pub fn check_claimed_limit<F>(&self, window_usage: u32, error_fn: F) -> Result<(), HttpError>
    where
        F: FnOnce(u32) -> RateLimitingError,
    {
        Self::check_inner(self.claimed_limit, window_usage, error_fn)
            .inspect(|()| {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::Create(
                    bencher_otel::IntervalKind::Day,
                    bencher_otel::AuthorizationKind::User,
                ));
            })
            .inspect_err(|_| {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::CreateMax(
                    bencher_otel::IntervalKind::Day,
                    bencher_otel::AuthorizationKind::User,
                ));
            })
    }

    fn check_inner<F>(limit: u32, window_usage: u32, error_fn: F) -> Result<(), HttpError>
    where
        F: FnOnce(u32) -> RateLimitingError,
    {
        if window_usage < limit {
            Ok(())
        } else {
            Err(too_many_requests(error_fn(limit)))
        }
    }

    pub fn public_request(&self, remote_ip: IpAddr) -> Result<(), HttpError> {
        self.public.check_request(remote_ip)
    }

    pub fn public_auth_attempt(&self, remote_ip: IpAddr) -> Result<(), HttpError> {
        self.public.check_attempt(remote_ip)
    }

    pub fn unclaimed_run(&self, remote_ip: IpAddr) -> Result<(), HttpError> {
        self.public.check_run(remote_ip)
    }

    pub fn user_request(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_request(user_uuid)
    }

    pub fn auth_attempt(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_attempt(user_uuid)
    }

    pub fn create_token(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_token(user_uuid)
    }

    pub fn create_organization(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_organization(user_uuid)
    }

    pub fn user_invite(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_invite(user_uuid)
    }

    pub fn claimed_run(&self, user_uuid: UserUuid) -> Result<(), HttpError> {
        self.user.check_run(user_uuid)
    }

    pub fn remote_ip(headers: &HeaderMap) -> Option<IpAddr> {
        remote_ip::remote_ip(headers)
    }
}

macro_rules! extract_rate_limits {
    ($opt:expr, $default_minute:expr, $default_hour:expr, $default_day:expr) => {{
        let minute = $opt.and_then(|r| r.minute).unwrap_or($default_minute);
        let hour = $opt.and_then(|r| r.hour).unwrap_or($default_hour);
        let day = $opt.and_then(|r| r.day).unwrap_or($default_day);
        RateLimits { minute, hour, day }
    }};
}

pub(crate) use extract_rate_limits;
