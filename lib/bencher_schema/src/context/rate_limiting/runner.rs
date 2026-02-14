use bencher_json::{RunnerUuid, system::config::JsonRunnerRateLimiter};

use crate::context::{
    RateLimitingError,
    rate_limiting::{RateLimiter, RateLimits, extract_rate_limits},
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

        let requests = extract_rate_limits!(
            requests,
            DEFAULT_REQUESTS_PER_MINUTE_LIMIT,
            DEFAULT_REQUESTS_PER_HOUR_LIMIT,
            DEFAULT_REQUESTS_PER_DAY_LIMIT
        );

        Self::new(requests)
    }
}

impl RunnerRateLimiter {
    pub fn new(requests: RateLimits) -> Self {
        let RateLimits { minute, hour, day } = requests;
        let requests = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &bencher_otel::ApiCounter::RunnerRequestMax,
            RateLimitingError::RunnerRequests,
        );

        Self { requests }
    }

    pub fn max() -> Self {
        let requests = RateLimits {
            minute: usize::MAX,
            hour: usize::MAX,
            day: usize::MAX,
        };

        Self::new(requests)
    }

    pub fn check_request(&self, runner_uuid: RunnerUuid) -> Result<(), dropshot::HttpError> {
        self.requests.check(runner_uuid)
    }
}
