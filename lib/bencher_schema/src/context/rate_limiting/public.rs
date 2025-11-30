use std::net::IpAddr;

use bencher_json::system::config::JsonPublicRateLimiter;

use crate::context::{
    RateLimitingError,
    rate_limiting::{RateLimiter, RateLimits},
};

const DEFAULT_REQUESTS_PER_MINUTE_LIMIT: usize = 1 << 10;
const DEFAULT_REQUESTS_PER_HOUR_LIMIT: usize = 1 << 12;
const DEFAULT_REQUESTS_PER_DAY_LIMIT: usize = 1 << 13;

const DEFAULT_ATTEMPTS_PER_MINUTE_LIMIT: usize = 1 << 5;
const DEFAULT_ATTEMPTS_PER_HOUR_LIMIT: usize = 1 << 6;
const DEFAULT_ATTEMPTS_PER_DAY_LIMIT: usize = 1 << 7;

const DEFAULT_RUNS_PER_MINUTE_LIMIT: usize = 1 << 5;
const DEFAULT_RUNS_PER_HOUR_LIMIT: usize = 1 << 7;
const DEFAULT_RUNS_PER_DAY_LIMIT: usize = 1 << 8;

pub(super) struct PublicRateLimiter {
    requests: RateLimiter<IpAddr>,
    attempts: RateLimiter<IpAddr>,
    runs: RateLimiter<IpAddr>,
}

impl Default for PublicRateLimiter {
    fn default() -> Self {
        let requests = RateLimits {
            minute: DEFAULT_REQUESTS_PER_MINUTE_LIMIT,
            hour: DEFAULT_REQUESTS_PER_HOUR_LIMIT,
            day: DEFAULT_REQUESTS_PER_DAY_LIMIT,
        };

        let attempts = RateLimits {
            minute: DEFAULT_ATTEMPTS_PER_MINUTE_LIMIT,
            hour: DEFAULT_ATTEMPTS_PER_HOUR_LIMIT,
            day: DEFAULT_ATTEMPTS_PER_DAY_LIMIT,
        };

        let runs = RateLimits {
            minute: DEFAULT_RUNS_PER_MINUTE_LIMIT,
            hour: DEFAULT_RUNS_PER_HOUR_LIMIT,
            day: DEFAULT_RUNS_PER_DAY_LIMIT,
        };

        Self::new(requests, attempts, runs)
    }
}

impl From<JsonPublicRateLimiter> for PublicRateLimiter {
    fn from(json: JsonPublicRateLimiter) -> Self {
        let JsonPublicRateLimiter {
            requests,
            attempts,
            runs,
        } = json;

        let minute = requests
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_REQUESTS_PER_MINUTE_LIMIT);
        let hour = requests
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_REQUESTS_PER_HOUR_LIMIT);
        let day = requests
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_REQUESTS_PER_DAY_LIMIT);
        let requests = RateLimits { minute, hour, day };

        let minute = attempts
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_ATTEMPTS_PER_MINUTE_LIMIT);
        let hour = attempts
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_ATTEMPTS_PER_HOUR_LIMIT);
        let day = attempts
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_ATTEMPTS_PER_DAY_LIMIT);
        let attempts = RateLimits { minute, hour, day };

        let minute = runs
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_RUNS_PER_MINUTE_LIMIT);
        let hour = runs
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_RUNS_PER_HOUR_LIMIT);
        let day = runs
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_RUNS_PER_DAY_LIMIT);
        let runs = RateLimits { minute, hour, day };

        Self::new(requests, attempts, runs)
    }
}

impl PublicRateLimiter {
    pub fn new(requests: RateLimits, attempts: RateLimits, runs: RateLimits) -> Self {
        let RateLimits { minute, hour, day } = requests;
        let requests = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::RequestMax(
                    interval,
                    bencher_otel::AuthorizationKind::Public,
                )
            },
            RateLimitingError::IpAddressRequests,
        );

        let RateLimits { minute, hour, day } = attempts;
        let attempts = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::UserAttemptMax(
                    interval,
                    bencher_otel::AuthorizationKind::Public,
                )
            },
            RateLimitingError::IpAddressRequests,
        );

        let RateLimits { minute, hour, day } = runs;
        let runs = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &bencher_otel::ApiCounter::RunUnclaimedMax,
            RateLimitingError::UnclaimedRun,
        );

        Self {
            requests,
            attempts,
            runs,
        }
    }

    pub fn check_request(&self, ip: IpAddr) -> Result<(), dropshot::HttpError> {
        self.requests.check(ip)
    }

    pub fn check_attempt(&self, ip: IpAddr) -> Result<(), dropshot::HttpError> {
        self.attempts.check(ip)
    }

    pub fn check_run(&self, ip: IpAddr) -> Result<(), dropshot::HttpError> {
        self.runs.check(ip)
    }
}
