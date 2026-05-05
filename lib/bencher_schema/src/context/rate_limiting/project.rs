use bencher_json::{ProjectUuid, system::config::JsonProjectRateLimiter};

use crate::context::{
    RateLimitingError,
    rate_limiting::{RateLimiter, RateLimits, extract_rate_limits},
};

const DEFAULT_RUNS_PER_MINUTE_LIMIT: usize = 1 << 6;
const DEFAULT_RUNS_PER_HOUR_LIMIT: usize = 1 << 10;
const DEFAULT_RUNS_PER_DAY_LIMIT: usize = 1 << 12;

pub(super) struct ProjectRateLimiter {
    runs: RateLimiter<ProjectUuid>,
}

impl Default for ProjectRateLimiter {
    fn default() -> Self {
        let runs = RateLimits {
            minute: DEFAULT_RUNS_PER_MINUTE_LIMIT,
            hour: DEFAULT_RUNS_PER_HOUR_LIMIT,
            day: DEFAULT_RUNS_PER_DAY_LIMIT,
        };

        Self::new(runs)
    }
}

impl From<JsonProjectRateLimiter> for ProjectRateLimiter {
    fn from(json: JsonProjectRateLimiter) -> Self {
        let JsonProjectRateLimiter { runs } = json;

        let runs = extract_rate_limits!(
            runs,
            DEFAULT_RUNS_PER_MINUTE_LIMIT,
            DEFAULT_RUNS_PER_HOUR_LIMIT,
            DEFAULT_RUNS_PER_DAY_LIMIT
        );

        Self::new(runs)
    }
}

impl ProjectRateLimiter {
    pub fn new(runs: RateLimits) -> Self {
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

        Self { runs }
    }

    pub fn max() -> Self {
        let runs = RateLimits {
            minute: usize::MAX,
            hour: usize::MAX,
            day: usize::MAX,
        };

        Self::new(runs)
    }

    pub fn check_run(&self, project_uuid: ProjectUuid) -> Result<(), dropshot::HttpError> {
        self.runs.check(project_uuid)
    }
}
