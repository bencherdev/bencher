use bencher_json::{UserUuid, system::config::JsonUserRateLimiter};
use bencher_rate_limiter::{RateLimiter, RateLimits};

use crate::context::{RateLimitingError, rate_limiting::snapshot::UserRateLimiterSnapshot};

const DEFAULT_REQUESTS_PER_MINUTE_LIMIT: usize = 1 << 11;
const DEFAULT_REQUESTS_PER_HOUR_LIMIT: usize = 1 << 13;
const DEFAULT_REQUESTS_PER_DAY_LIMIT: usize = 1 << 14;

const DEFAULT_ATTEMPTS_PER_MINUTE_LIMIT: usize = 1 << 1;
const DEFAULT_ATTEMPTS_PER_HOUR_LIMIT: usize = 1 << 2;
const DEFAULT_ATTEMPTS_PER_DAY_LIMIT: usize = 1 << 3;

const DEFAULT_CREDENTIALS_PER_MINUTE_LIMIT: usize = 1 << 2;
const DEFAULT_CREDENTIALS_PER_HOUR_LIMIT: usize = 1 << 3;
const DEFAULT_CREDENTIALS_PER_DAY_LIMIT: usize = 1 << 4;

const DEFAULT_ORGANIZATIONS_PER_MINUTE_LIMIT: usize = 1 << 1;
const DEFAULT_ORGANIZATIONS_PER_HOUR_LIMIT: usize = 1 << 2;
const DEFAULT_ORGANIZATIONS_PER_DAY_LIMIT: usize = 1 << 3;

const DEFAULT_INVITES_PER_MINUTE_LIMIT: usize = 1 << 3;
const DEFAULT_INVITES_PER_HOUR_LIMIT: usize = 1 << 4;
const DEFAULT_INVITES_PER_DAY_LIMIT: usize = 1 << 5;

const DEFAULT_RUNS_PER_MINUTE_LIMIT: usize = 1 << 6;
const DEFAULT_RUNS_PER_HOUR_LIMIT: usize = 1 << 10;
const DEFAULT_RUNS_PER_DAY_LIMIT: usize = 1 << 12;

pub(super) struct UserRateLimiter {
    requests: RateLimiter<UserUuid>,
    attempts: RateLimiter<UserUuid>,
    credentials: RateLimiter<UserUuid>,
    organizations: RateLimiter<UserUuid>,
    invites: RateLimiter<UserUuid>,
    runs: RateLimiter<UserUuid>,
}

impl Default for UserRateLimiter {
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

        let credentials = RateLimits {
            minute: DEFAULT_CREDENTIALS_PER_MINUTE_LIMIT,
            hour: DEFAULT_CREDENTIALS_PER_HOUR_LIMIT,
            day: DEFAULT_CREDENTIALS_PER_DAY_LIMIT,
        };

        let organizations = RateLimits {
            minute: DEFAULT_ORGANIZATIONS_PER_MINUTE_LIMIT,
            hour: DEFAULT_ORGANIZATIONS_PER_HOUR_LIMIT,
            day: DEFAULT_ORGANIZATIONS_PER_DAY_LIMIT,
        };

        let invites = RateLimits {
            minute: DEFAULT_INVITES_PER_MINUTE_LIMIT,
            hour: DEFAULT_INVITES_PER_HOUR_LIMIT,
            day: DEFAULT_INVITES_PER_DAY_LIMIT,
        };

        let runs = RateLimits {
            minute: DEFAULT_RUNS_PER_MINUTE_LIMIT,
            hour: DEFAULT_RUNS_PER_HOUR_LIMIT,
            day: DEFAULT_RUNS_PER_DAY_LIMIT,
        };

        Self::new(
            requests,
            attempts,
            credentials,
            organizations,
            invites,
            runs,
        )
    }
}

impl From<JsonUserRateLimiter> for UserRateLimiter {
    fn from(json: JsonUserRateLimiter) -> Self {
        let JsonUserRateLimiter {
            requests,
            attempts,
            credentials,
            organizations,
            invites,
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

        let credentials = RateLimits {
            minute: credentials
                .and_then(|r| r.minute)
                .unwrap_or(DEFAULT_CREDENTIALS_PER_MINUTE_LIMIT),
            hour: credentials
                .and_then(|r| r.hour)
                .unwrap_or(DEFAULT_CREDENTIALS_PER_HOUR_LIMIT),
            day: credentials
                .and_then(|r| r.day)
                .unwrap_or(DEFAULT_CREDENTIALS_PER_DAY_LIMIT),
        };

        let organizations = RateLimits {
            minute: organizations
                .and_then(|r| r.minute)
                .unwrap_or(DEFAULT_ORGANIZATIONS_PER_MINUTE_LIMIT),
            hour: organizations
                .and_then(|r| r.hour)
                .unwrap_or(DEFAULT_ORGANIZATIONS_PER_HOUR_LIMIT),
            day: organizations
                .and_then(|r| r.day)
                .unwrap_or(DEFAULT_ORGANIZATIONS_PER_DAY_LIMIT),
        };

        let invites = RateLimits {
            minute: invites
                .and_then(|r| r.minute)
                .unwrap_or(DEFAULT_INVITES_PER_MINUTE_LIMIT),
            hour: invites
                .and_then(|r| r.hour)
                .unwrap_or(DEFAULT_INVITES_PER_HOUR_LIMIT),
            day: invites
                .and_then(|r| r.day)
                .unwrap_or(DEFAULT_INVITES_PER_DAY_LIMIT),
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

        Self::new(
            requests,
            attempts,
            credentials,
            organizations,
            invites,
            runs,
        )
    }
}

impl UserRateLimiter {
    pub fn new(
        requests: RateLimits,
        attempts: RateLimits,
        credentials: RateLimits,
        organizations: RateLimits,
        invites: RateLimits,
        runs: RateLimits,
    ) -> Self {
        Self {
            requests: RateLimiter::new(requests),
            attempts: RateLimiter::new(attempts),
            credentials: RateLimiter::new(credentials),
            organizations: RateLimiter::new(organizations),
            invites: RateLimiter::new(invites),
            runs: RateLimiter::new(runs),
        }
    }

    pub fn max() -> Self {
        let max = RateLimits {
            minute: usize::MAX,
            hour: usize::MAX,
            day: usize::MAX,
        };

        Self::new(max, max, max, max, max, max)
    }

    pub fn prune(&self) {
        self.requests.prune();
        self.attempts.prune();
        self.credentials.prune();
        self.organizations.prune();
        self.invites.prune();
        self.runs.prune();
    }

    pub fn snapshot(&self) -> UserRateLimiterSnapshot {
        UserRateLimiterSnapshot {
            requests: self.requests.snapshot(),
            attempts: self.attempts.snapshot(),
            credentials: self.credentials.snapshot(),
            organizations: self.organizations.snapshot(),
            invites: self.invites.snapshot(),
            runs: self.runs.snapshot(),
        }
    }

    pub fn restore(&self, snapshot: UserRateLimiterSnapshot) {
        let UserRateLimiterSnapshot {
            requests,
            attempts,
            credentials,
            organizations,
            invites,
            runs,
        } = snapshot;
        self.requests.restore(requests);
        self.attempts.restore(attempts);
        self.credentials.restore(credentials);
        self.organizations.restore(organizations);
        self.invites.restore(invites);
        self.runs.restore(runs);
    }

    pub fn check_request(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        if self.requests.check(user_uuid) {
            Ok(())
        } else {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RequestMax(
                bencher_otel::IntervalKind::Minute,
                bencher_otel::AuthorizationKind::User,
            ));
            Err(crate::error::too_many_requests(
                RateLimitingError::UserRequests,
            ))
        }
    }

    pub fn check_attempt(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        if self.attempts.check(user_uuid) {
            Ok(())
        } else {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserAttemptMax(
                bencher_otel::IntervalKind::Minute,
                bencher_otel::AuthorizationKind::User,
            ));
            Err(crate::error::too_many_requests(
                RateLimitingError::UserAttempts,
            ))
        }
    }

    pub fn check_credential(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        if self.credentials.check(user_uuid) {
            Ok(())
        } else {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserCredentialMax(
                bencher_otel::IntervalKind::Minute,
            ));
            Err(crate::error::too_many_requests(
                RateLimitingError::UserCredentials,
            ))
        }
    }

    pub fn check_organization(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        if self.organizations.check(user_uuid) {
            Ok(())
        } else {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserOrganizationMax(
                bencher_otel::IntervalKind::Minute,
            ));
            Err(crate::error::too_many_requests(
                RateLimitingError::UserOrganizations,
            ))
        }
    }

    pub fn check_invite(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        if self.invites.check(user_uuid) {
            Ok(())
        } else {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserInviteMax(
                bencher_otel::IntervalKind::Minute,
            ));
            Err(crate::error::too_many_requests(
                RateLimitingError::UserInvites,
            ))
        }
    }

    pub fn check_run(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        if self.runs.check(user_uuid) {
            Ok(())
        } else {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunClaimedMax(
                bencher_otel::IntervalKind::Minute,
            ));
            Err(crate::error::too_many_requests(RateLimitingError::UserRuns))
        }
    }
}
