use std::net::IpAddr;

use bencher_json::system::config::JsonPublicRateLimiter;
use bencher_rate_limiter::{RateLimiter, RateLimits};

use crate::context::{RateLimitingError, rate_limiting::snapshot::PublicRateLimiterSnapshot};

const DEFAULT_REQUESTS_PER_MINUTE_LIMIT: usize = 1 << 10;
const DEFAULT_REQUESTS_PER_HOUR_LIMIT: usize = 1 << 12;
const DEFAULT_REQUESTS_PER_DAY_LIMIT: usize = 1 << 13;

const DEFAULT_ATTEMPTS_PER_MINUTE_LIMIT: usize = 1 << 5;
const DEFAULT_ATTEMPTS_PER_HOUR_LIMIT: usize = 1 << 6;
const DEFAULT_ATTEMPTS_PER_DAY_LIMIT: usize = 1 << 7;

const DEFAULT_RUNS_PER_MINUTE_LIMIT: usize = 1 << 6;
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

        let requests = RateLimits {
            minute: requests
                .and_then(|r| r.minute)
                .unwrap_or(DEFAULT_REQUESTS_PER_MINUTE_LIMIT),
            hour: requests
                .and_then(|r| r.hour)
                .unwrap_or(DEFAULT_REQUESTS_PER_HOUR_LIMIT),
            day: requests
                .and_then(|r| r.day)
                .unwrap_or(DEFAULT_REQUESTS_PER_DAY_LIMIT),
        };

        let attempts = RateLimits {
            minute: attempts
                .and_then(|r| r.minute)
                .unwrap_or(DEFAULT_ATTEMPTS_PER_MINUTE_LIMIT),
            hour: attempts
                .and_then(|r| r.hour)
                .unwrap_or(DEFAULT_ATTEMPTS_PER_HOUR_LIMIT),
            day: attempts
                .and_then(|r| r.day)
                .unwrap_or(DEFAULT_ATTEMPTS_PER_DAY_LIMIT),
        };

        let runs = RateLimits {
            minute: runs
                .and_then(|r| r.minute)
                .unwrap_or(DEFAULT_RUNS_PER_MINUTE_LIMIT),
            hour: runs
                .and_then(|r| r.hour)
                .unwrap_or(DEFAULT_RUNS_PER_HOUR_LIMIT),
            day: runs
                .and_then(|r| r.day)
                .unwrap_or(DEFAULT_RUNS_PER_DAY_LIMIT),
        };

        Self::new(requests, attempts, runs)
    }
}

impl PublicRateLimiter {
    pub fn new(requests: RateLimits, attempts: RateLimits, runs: RateLimits) -> Self {
        Self {
            requests: RateLimiter::new(requests),
            attempts: RateLimiter::new(attempts),
            runs: RateLimiter::new(runs),
        }
    }

    pub fn max() -> Self {
        let max = RateLimits {
            minute: usize::MAX,
            hour: usize::MAX,
            day: usize::MAX,
        };

        Self::new(max, max, max)
    }

    pub fn prune(&self) {
        self.requests.prune();
        self.attempts.prune();
        self.runs.prune();
    }

    pub fn snapshot(&self) -> PublicRateLimiterSnapshot {
        PublicRateLimiterSnapshot {
            requests: self.requests.snapshot(),
            attempts: self.attempts.snapshot(),
            runs: self.runs.snapshot(),
        }
    }

    pub fn restore(&self, snapshot: PublicRateLimiterSnapshot) {
        let PublicRateLimiterSnapshot {
            requests,
            attempts,
            runs,
        } = snapshot;
        self.requests.restore(requests);
        self.attempts.restore(attempts);
        self.runs.restore(runs);
    }

    pub fn check_request(&self, ip: IpAddr) -> Result<(), dropshot::HttpError> {
        if let Some(interval) = self.requests.check(ip) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RequestMax(
                super::interval_kind(interval),
                bencher_otel::AuthorizationKind::Public,
            ));
            Err(crate::error::too_many_requests(
                RateLimitingError::IpAddressRequests,
            ))
        } else {
            Ok(())
        }
    }

    pub fn check_attempt(&self, ip: IpAddr) -> Result<(), dropshot::HttpError> {
        if let Some(interval) = self.attempts.check(ip) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserAttemptMax(
                super::interval_kind(interval),
                bencher_otel::AuthorizationKind::Public,
            ));
            Err(crate::error::too_many_requests(
                RateLimitingError::IpAddressRequests,
            ))
        } else {
            Ok(())
        }
    }

    pub fn check_run(&self, ip: IpAddr) -> Result<(), dropshot::HttpError> {
        if let Some(interval) = self.runs.check(ip) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunUnclaimedMax(
                super::interval_kind(interval),
            ));
            Err(crate::error::too_many_requests(
                RateLimitingError::UnclaimedRun,
            ))
        } else {
            Ok(())
        }
    }
}
