use bencher_json::{UserUuid, system::config::JsonUserRateLimiter};

use crate::context::{
    RateLimitingError,
    rate_limiting::{RateLimiter, RateLimits, extract_rate_limits},
};

const DEFAULT_REQUESTS_PER_MINUTE_LIMIT: usize = 1 << 11;
const DEFAULT_REQUESTS_PER_HOUR_LIMIT: usize = 1 << 13;
const DEFAULT_REQUESTS_PER_DAY_LIMIT: usize = 1 << 14;

const DEFAULT_ATTEMPTS_PER_MINUTE_LIMIT: usize = 1 << 1;
const DEFAULT_ATTEMPTS_PER_HOUR_LIMIT: usize = 1 << 2;
const DEFAULT_ATTEMPTS_PER_DAY_LIMIT: usize = 1 << 3;

const DEFAULT_TOKENS_PER_MINUTE_LIMIT: usize = 1 << 2;
const DEFAULT_TOKENS_PER_HOUR_LIMIT: usize = 1 << 3;
const DEFAULT_TOKENS_PER_DAY_LIMIT: usize = 1 << 4;

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
    tokens: RateLimiter<UserUuid>,
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

        let tokens = RateLimits {
            minute: DEFAULT_TOKENS_PER_MINUTE_LIMIT,
            hour: DEFAULT_TOKENS_PER_HOUR_LIMIT,
            day: DEFAULT_TOKENS_PER_DAY_LIMIT,
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

        Self::new(requests, attempts, tokens, organizations, invites, runs)
    }
}

impl From<JsonUserRateLimiter> for UserRateLimiter {
    fn from(json: JsonUserRateLimiter) -> Self {
        let JsonUserRateLimiter {
            requests,
            attempts,
            tokens,
            organizations,
            invites,
            runs,
        } = json;

        let requests = extract_rate_limits!(
            requests,
            DEFAULT_REQUESTS_PER_MINUTE_LIMIT,
            DEFAULT_REQUESTS_PER_HOUR_LIMIT,
            DEFAULT_REQUESTS_PER_DAY_LIMIT
        );

        let attempts = extract_rate_limits!(
            attempts,
            DEFAULT_ATTEMPTS_PER_MINUTE_LIMIT,
            DEFAULT_ATTEMPTS_PER_HOUR_LIMIT,
            DEFAULT_ATTEMPTS_PER_DAY_LIMIT
        );

        let tokens = extract_rate_limits!(
            tokens,
            DEFAULT_TOKENS_PER_MINUTE_LIMIT,
            DEFAULT_TOKENS_PER_HOUR_LIMIT,
            DEFAULT_TOKENS_PER_DAY_LIMIT
        );

        let organizations = extract_rate_limits!(
            organizations,
            DEFAULT_ORGANIZATIONS_PER_MINUTE_LIMIT,
            DEFAULT_ORGANIZATIONS_PER_HOUR_LIMIT,
            DEFAULT_ORGANIZATIONS_PER_DAY_LIMIT
        );

        let invites = extract_rate_limits!(
            invites,
            DEFAULT_INVITES_PER_MINUTE_LIMIT,
            DEFAULT_INVITES_PER_HOUR_LIMIT,
            DEFAULT_INVITES_PER_DAY_LIMIT
        );

        let runs = extract_rate_limits!(
            runs,
            DEFAULT_RUNS_PER_MINUTE_LIMIT,
            DEFAULT_RUNS_PER_HOUR_LIMIT,
            DEFAULT_RUNS_PER_DAY_LIMIT
        );

        Self::new(requests, attempts, tokens, organizations, invites, runs)
    }
}

impl UserRateLimiter {
    pub fn new(
        requests: RateLimits,
        attempts: RateLimits,
        tokens: RateLimits,
        organizations: RateLimits,
        invites: RateLimits,
        runs: RateLimits,
    ) -> Self {
        let RateLimits { minute, hour, day } = requests;
        let requests = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::RequestMax(
                    interval,
                    bencher_otel::AuthorizationKind::User,
                )
            },
            RateLimitingError::UserRequests,
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
                    bencher_otel::AuthorizationKind::User,
                )
            },
            RateLimitingError::UserAttempts,
        );

        let RateLimits { minute, hour, day } = tokens;
        let tokens = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &bencher_otel::ApiCounter::UserTokenMax,
            RateLimitingError::UserTokens,
        );

        let RateLimits { minute, hour, day } = organizations;
        let organizations = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &bencher_otel::ApiCounter::UserOrganizationMax,
            RateLimitingError::UserOrganizations,
        );

        let RateLimits { minute, hour, day } = invites;
        let invites = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &bencher_otel::ApiCounter::UserInviteMax,
            RateLimitingError::UserInvites,
        );

        let RateLimits { minute, hour, day } = runs;
        let runs = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &bencher_otel::ApiCounter::RunClaimedMax,
            RateLimitingError::UserRequests,
        );

        Self {
            requests,
            attempts,
            tokens,
            organizations,
            invites,
            runs,
        }
    }

    pub fn check_request(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.requests.check(user_uuid)
    }

    pub fn check_attempt(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.attempts.check(user_uuid)
    }

    pub fn check_token(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.tokens.check(user_uuid)
    }

    pub fn check_organization(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.organizations.check(user_uuid)
    }

    pub fn check_invite(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.invites.check(user_uuid)
    }

    pub fn check_run(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.runs.check(user_uuid)
    }
}
