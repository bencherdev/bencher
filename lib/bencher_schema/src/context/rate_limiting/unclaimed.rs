use std::net::IpAddr;

use bencher_json::system::config::JsonUnclaimedRateLimiter;

use crate::context::{
    RateLimitingError,
    rate_limiting::{RateLimiter, RateLimits},
};

const DEFAULT_RUN_MINUTE_LIMIT: usize = 1 << 5;
const DEFAULT_RUN_HOUR_LIMIT: usize = 1 << 7;
const DEFAULT_RUN_DAY_LIMIT: usize = 1 << 8;

pub(super) struct UnclaimedRateLimiter {
    run: RateLimiter<IpAddr>,
}

impl Default for UnclaimedRateLimiter {
    fn default() -> Self {
        let run = RateLimits {
            minute: DEFAULT_RUN_MINUTE_LIMIT,
            hour: DEFAULT_RUN_HOUR_LIMIT,
            day: DEFAULT_RUN_DAY_LIMIT,
        };

        Self::new(run)
    }
}

impl From<JsonUnclaimedRateLimiter> for UnclaimedRateLimiter {
    fn from(json: JsonUnclaimedRateLimiter) -> Self {
        let minute = json
            .run
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_RUN_MINUTE_LIMIT);
        let hour = json
            .run
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_RUN_HOUR_LIMIT);
        let day = json
            .run
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_RUN_DAY_LIMIT);
        let run = RateLimits { minute, hour, day };

        Self::new(run)
    }
}

impl UnclaimedRateLimiter {
    pub fn new(run: RateLimits) -> Self {
        let RateLimits { minute, hour, day } = run;
        let run = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::UnclaimedMax(interval, bencher_otel::UnclaimedKind::Run)
            },
            RateLimitingError::UnclaimedRun,
        );

        Self { run }
    }

    pub fn check_run(&self, ip: IpAddr) -> Result<(), dropshot::HttpError> {
        self.run.check(ip)
    }
}
