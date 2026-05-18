use bencher_json::{ProjectUuid, system::config::JsonProjectRateLimiter};
use bencher_rate_limiter::{RateLimiter, RateLimits};

#[cfg(feature = "otel")]
use super::interval_kind;
use crate::{
    context::{RateLimitingError, rate_limiting::snapshot::ProjectRateLimiterSnapshot},
    error::too_many_requests,
};

const DEFAULT_REQUESTS_PER_MINUTE_LIMIT: usize = 1 << 11;
const DEFAULT_REQUESTS_PER_HOUR_LIMIT: usize = 1 << 13;
const DEFAULT_REQUESTS_PER_DAY_LIMIT: usize = 1 << 14;

const DEFAULT_RUNS_PER_MINUTE_LIMIT: usize = 1 << 6;
const DEFAULT_RUNS_PER_HOUR_LIMIT: usize = 1 << 10;
const DEFAULT_RUNS_PER_DAY_LIMIT: usize = 1 << 12;

pub(super) struct ProjectRateLimiter {
    requests: RateLimiter<ProjectUuid>,
    runs: RateLimiter<ProjectUuid>,
}

impl Default for ProjectRateLimiter {
    fn default() -> Self {
        let requests = RateLimits {
            minute: DEFAULT_REQUESTS_PER_MINUTE_LIMIT,
            hour: DEFAULT_REQUESTS_PER_HOUR_LIMIT,
            day: DEFAULT_REQUESTS_PER_DAY_LIMIT,
        };

        let runs = RateLimits {
            minute: DEFAULT_RUNS_PER_MINUTE_LIMIT,
            hour: DEFAULT_RUNS_PER_HOUR_LIMIT,
            day: DEFAULT_RUNS_PER_DAY_LIMIT,
        };

        Self::new(requests, runs)
    }
}

impl From<JsonProjectRateLimiter> for ProjectRateLimiter {
    fn from(json: JsonProjectRateLimiter) -> Self {
        let JsonProjectRateLimiter { requests, runs } = json;

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

        Self::new(requests, runs)
    }
}

impl ProjectRateLimiter {
    pub fn new(requests: RateLimits, runs: RateLimits) -> Self {
        Self {
            requests: RateLimiter::new(requests),
            runs: RateLimiter::new(runs),
        }
    }

    pub fn max() -> Self {
        let max = RateLimits {
            minute: usize::MAX,
            hour: usize::MAX,
            day: usize::MAX,
        };

        Self::new(max, max)
    }

    pub fn prune(&self) {
        self.requests.prune();
        self.runs.prune();
    }

    pub fn snapshot(&self) -> ProjectRateLimiterSnapshot {
        ProjectRateLimiterSnapshot {
            requests: self.requests.snapshot(),
            runs: self.runs.snapshot(),
        }
    }

    pub fn restore(&self, snapshot: ProjectRateLimiterSnapshot) {
        let ProjectRateLimiterSnapshot { requests, runs } = snapshot;
        self.requests.restore(requests);
        self.runs.restore(runs);
    }

    pub fn check_request(&self, project_uuid: ProjectUuid) -> Result<(), dropshot::HttpError> {
        if let Some(interval) = self.requests.check(project_uuid) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RequestMax(
                interval_kind(interval),
                bencher_otel::AuthorizationKind::Project,
            ));
            Err(too_many_requests(RateLimitingError::ProjectRequests(
                interval,
            )))
        } else {
            Ok(())
        }
    }

    pub fn check_run(&self, project_uuid: ProjectUuid) -> Result<(), dropshot::HttpError> {
        if let Some(interval) = self.runs.check(project_uuid) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RequestMax(
                interval_kind(interval),
                bencher_otel::AuthorizationKind::Project,
            ));
            Err(too_many_requests(RateLimitingError::ProjectRuns(interval)))
        } else {
            Ok(())
        }
    }
}
