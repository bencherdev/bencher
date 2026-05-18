use bencher_json::{RunnerUuid, system::config::JsonRunnerRateLimiter};
use bencher_rate_limiter::{RateLimiter, RateLimits};

#[cfg(feature = "otel")]
use super::interval_kind;
use crate::{
    context::{RateLimitingError, rate_limiting::snapshot::RunnerRateLimiterSnapshot},
    error::too_many_requests,
};

const DEFAULT_REQUESTS_PER_MINUTE_LIMIT: usize = 1 << 4;
const DEFAULT_REQUESTS_PER_HOUR_LIMIT: usize = 1 << 8;
const DEFAULT_REQUESTS_PER_DAY_LIMIT: usize = 1 << 12;

pub(super) struct RunnerRateLimiter {
    requests: RateLimiter<RunnerUuid>,
}

impl Default for RunnerRateLimiter {
    fn default() -> Self {
        let requests = RateLimits {
            minute: DEFAULT_REQUESTS_PER_MINUTE_LIMIT,
            hour: DEFAULT_REQUESTS_PER_HOUR_LIMIT,
            day: DEFAULT_REQUESTS_PER_DAY_LIMIT,
        };

        Self::new(requests)
    }
}

impl From<JsonRunnerRateLimiter> for RunnerRateLimiter {
    fn from(json: JsonRunnerRateLimiter) -> Self {
        let JsonRunnerRateLimiter { requests } = json;

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

        Self::new(requests)
    }
}

impl RunnerRateLimiter {
    pub fn new(requests: RateLimits) -> Self {
        Self {
            requests: RateLimiter::new(requests),
        }
    }

    pub fn max() -> Self {
        let requests = RateLimits {
            minute: usize::MAX,
            hour: usize::MAX,
            day: usize::MAX,
        };

        Self::new(requests)
    }

    pub fn prune(&self) {
        self.requests.prune();
    }

    pub fn snapshot(&self) -> RunnerRateLimiterSnapshot {
        RunnerRateLimiterSnapshot {
            requests: self.requests.snapshot(),
        }
    }

    pub fn restore(&self, snapshot: RunnerRateLimiterSnapshot) {
        let RunnerRateLimiterSnapshot { requests } = snapshot;
        self.requests.restore(requests);
    }

    pub fn check_request(&self, runner_uuid: RunnerUuid) -> Result<(), dropshot::HttpError> {
        if let Some(interval) = self.requests.check(runner_uuid) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerRequestMax(
                interval_kind(interval),
            ));
            Err(too_many_requests(RateLimitingError::RunnerRequests(
                interval,
            )))
        } else {
            Ok(())
        }
    }
}
