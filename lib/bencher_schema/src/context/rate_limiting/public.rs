use std::net::IpAddr;

use bencher_json::system::config::JsonPublicRateLimiter;
use bencher_rate_limiter::{RateLimiter, RateLimits};
use http::{HeaderValue, header::RETRY_AFTER};

use super::IpKey;
#[cfg(feature = "otel")]
use super::interval_kind;
use crate::{
    context::{RateLimitingError, rate_limiting::snapshot::PublicRateLimiterSnapshot},
    error::too_many_requests,
};

const DEFAULT_REQUESTS_PER_MINUTE_LIMIT: usize = 1 << 10;
const DEFAULT_REQUESTS_PER_HOUR_LIMIT: usize = 1 << 12;
const DEFAULT_REQUESTS_PER_DAY_LIMIT: usize = 1 << 13;

const DEFAULT_ATTEMPTS_PER_MINUTE_LIMIT: usize = 1 << 5;
const DEFAULT_ATTEMPTS_PER_HOUR_LIMIT: usize = 1 << 6;
const DEFAULT_ATTEMPTS_PER_DAY_LIMIT: usize = 1 << 7;

const DEFAULT_RUNS_PER_MINUTE_LIMIT: usize = 1 << 6;
const DEFAULT_RUNS_PER_HOUR_LIMIT: usize = 1 << 7;
const DEFAULT_RUNS_PER_DAY_LIMIT: usize = 1 << 8;

/// The limits on requests from one reader, keyed as [`IpKey`] keys an address.
pub(super) struct PublicRateLimiter {
    requests: RateLimiter<IpKey>,
    attempts: RateLimiter<IpKey>,
    runs: RateLimiter<IpKey>,
}

impl Default for PublicRateLimiter {
    fn default() -> Self {
        let requests = request_limits(None);

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
        let requests = request_limits(Some(json));
        let JsonPublicRateLimiter {
            requests: _,
            attempts,
            runs,
        } = json;

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

    pub fn prune(&self) -> usize {
        self.requests.prune() + self.attempts.prune() + self.runs.prune()
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
        if let Some(interval) = self.requests.check(IpKey::from(ip)) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RequestMax(
                interval_kind(interval),
                bencher_otel::AuthorizationKind::Public,
            ));
            Err(too_many_requests(RateLimitingError::IpAddressRequests(
                interval,
            )))
        } else {
            Ok(())
        }
    }

    pub fn check_attempt(&self, ip: IpAddr) -> Result<(), dropshot::HttpError> {
        let ip = IpKey::from(ip);
        if let Some(interval) = self.attempts.check(ip) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserAttemptMax(
                interval_kind(interval),
                bencher_otel::AuthorizationKind::Public,
            ));
            let mut error = too_many_requests(RateLimitingError::IpAddressAttempts(interval));
            if let Some(wait) = self.attempts.wait(ip) {
                error
                    .headers_mut()
                    .insert(RETRY_AFTER, HeaderValue::from(wait.as_secs()));
            }
            Err(error)
        } else {
            Ok(())
        }
    }

    pub fn check_run(&self, ip: IpAddr) -> Result<(), dropshot::HttpError> {
        if let Some(interval) = self.runs.check(IpKey::from(ip)) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunUnclaimedMax(
                interval_kind(interval),
            ));
            Err(too_many_requests(RateLimitingError::UnclaimedRun(interval)))
        } else {
            Ok(())
        }
    }
}

/// The limits on requests from one address, as configured.
pub(super) fn request_limits(public: Option<JsonPublicRateLimiter>) -> RateLimits {
    let requests = public.and_then(|public| public.requests);
    RateLimits {
        minute: requests
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_REQUESTS_PER_MINUTE_LIMIT),
        hour: requests
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_REQUESTS_PER_HOUR_LIMIT),
        day: requests
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_REQUESTS_PER_DAY_LIMIT),
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use bencher_rate_limiter::RateLimits;
    use dropshot::HttpError;

    use super::PublicRateLimiter;

    type Check = fn(&PublicRateLimiter, IpAddr) -> Result<(), HttpError>;

    const CHECKS: [Check; 3] = [
        PublicRateLimiter::check_request,
        PublicRateLimiter::check_attempt,
        PublicRateLimiter::check_run,
    ];

    // An IPv6 reader is one reader across its /64, the block one subscriber holds, and another
    // /64 is another reader.
    #[test]
    fn ipv6_readers_share_their_64() {
        for check in CHECKS {
            let limiter = one_each();
            check(&limiter, ip("2001:db8:1:2::1")).unwrap();
            assert!(check(&limiter, ip("2001:db8:1:2:ffff:ffff:ffff:ffff")).is_err());
            check(&limiter, ip("2001:db8:1:3::1")).unwrap();
        }
    }

    // An IPv4 reader is one reader whether its address comes as IPv4 or as IPv4-mapped IPv6.
    #[test]
    fn mapped_ipv4_shares_the_ipv4_bucket() {
        for check in CHECKS {
            let limiter = one_each();
            check(&limiter, ip("203.0.113.7")).unwrap();
            assert!(check(&limiter, ip("::ffff:203.0.113.7")).is_err());
            check(&limiter, ip("203.0.113.8")).unwrap();
        }
    }

    /// Room for one of each a minute, for each reader.
    fn one_each() -> PublicRateLimiter {
        let one = RateLimits {
            minute: 1,
            hour: usize::MAX,
            day: usize::MAX,
        };
        PublicRateLimiter::new(one, one, one)
    }

    fn ip(address: &str) -> IpAddr {
        address.parse().unwrap()
    }
}
