use bencher_json::{ProjectUuid, system::config::JsonProjectRateLimiter};

use crate::context::{
    RateLimitingError,
    rate_limiting::{
        RateLimiter, RateLimits, extract_rate_limits, snapshot::ProjectRateLimiterSnapshot,
    },
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

        let requests = extract_rate_limits!(
            requests,
            DEFAULT_REQUESTS_PER_MINUTE_LIMIT,
            DEFAULT_REQUESTS_PER_HOUR_LIMIT,
            DEFAULT_REQUESTS_PER_DAY_LIMIT
        );

        let runs = extract_rate_limits!(
            runs,
            DEFAULT_RUNS_PER_MINUTE_LIMIT,
            DEFAULT_RUNS_PER_HOUR_LIMIT,
            DEFAULT_RUNS_PER_DAY_LIMIT
        );

        Self::new(requests, runs)
    }
}

impl ProjectRateLimiter {
    pub fn new(requests: RateLimits, runs: RateLimits) -> Self {
        let RateLimits { minute, hour, day } = requests;
        let requests = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::RequestMax(
                    interval,
                    bencher_otel::AuthorizationKind::Project,
                )
            },
            RateLimitingError::ProjectRequests,
        );

        let RateLimits { minute, hour, day } = runs;
        let runs = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::RequestMax(
                    interval,
                    bencher_otel::AuthorizationKind::Project,
                )
            },
            RateLimitingError::ProjectRuns,
        );

        Self { requests, runs }
    }

    pub fn max() -> Self {
        let requests = RateLimits {
            minute: usize::MAX,
            hour: usize::MAX,
            day: usize::MAX,
        };

        let runs = RateLimits {
            minute: usize::MAX,
            hour: usize::MAX,
            day: usize::MAX,
        };

        Self::new(requests, runs)
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
        self.requests.check(project_uuid)
    }

    pub fn check_run(&self, project_uuid: ProjectUuid) -> Result<(), dropshot::HttpError> {
        self.runs.check(project_uuid)
    }
}
