use bencher_json::{UserUuid, system::config::JsonUserRateLimiter};

use crate::context::{
    RateLimitingError,
    rate_limiting::{RateLimiter, RateLimits},
};

const DEFAULT_REQUESTS_PER_MINUTE_LIMIT: usize = 1 << 11;
const DEFAULT_REQUESTS_PER_HOUR_LIMIT: usize = 1 << 13;
const DEFAULT_REQUESTS_PER_DAY_LIMIT: usize = 1 << 14;

const DEFAULT_ATTEMPTS_PER_MINUTE_LIMIT: usize = 1 << 1;
const DEFAULT_ATTEMPTS_PER_HOUR_LIMIT: usize = 1 << 2;
const DEFAULT_ATTEMPTS_PER_DAY_LIMIT: usize = 1 << 3;

const DEFAULT_ORGANIZATIONS_PER_MINUTE_LIMIT: usize = 1 << 1;
const DEFAULT_ORGANIZATIONS_PER_HOUR_LIMIT: usize = 1 << 2;
const DEFAULT_ORGANIZATIONS_PER_DAY_LIMIT: usize = 1 << 3;

const DEFAULT_INVITES_PER_MINUTE_LIMIT: usize = 1 << 3;
const DEFAULT_INVITES_PER_HOUR_LIMIT: usize = 1 << 4;
const DEFAULT_INVITES_PER_DAY_LIMIT: usize = 1 << 5;

pub(super) struct UserRateLimiter {
    requests: RateLimiter<UserUuid>,
    attempts: RateLimiter<UserUuid>,
    organizations: RateLimiter<UserUuid>,
    invites: RateLimiter<UserUuid>,
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

        Self::new(requests, attempts, organizations, invites)
    }
}

impl From<JsonUserRateLimiter> for UserRateLimiter {
    fn from(json: JsonUserRateLimiter) -> Self {
        let JsonUserRateLimiter {
            requests,
            attempts,
            organizations,
            invites,
        } = json;

        let minute = requests
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_REQUESTS_PER_MINUTE_LIMIT);
        let hour = requests
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_REQUESTS_PER_HOUR_LIMIT);
        let day = requests
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_REQUESTS_PER_DAY_LIMIT);
        let requests = RateLimits { minute, hour, day };

        let minute = attempts
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_ATTEMPTS_PER_MINUTE_LIMIT);
        let hour = attempts
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_ATTEMPTS_PER_HOUR_LIMIT);
        let day = attempts
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_ATTEMPTS_PER_DAY_LIMIT);
        let attempts = RateLimits { minute, hour, day };

        let minute = organizations
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_ORGANIZATIONS_PER_MINUTE_LIMIT);
        let hour = organizations
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_ORGANIZATIONS_PER_HOUR_LIMIT);
        let day = organizations
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_ORGANIZATIONS_PER_DAY_LIMIT);
        let organizations = RateLimits { minute, hour, day };

        let minute = invites
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_INVITES_PER_MINUTE_LIMIT);
        let hour = invites
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_INVITES_PER_HOUR_LIMIT);
        let day = invites
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_INVITES_PER_DAY_LIMIT);
        let invites = RateLimits { minute, hour, day };

        Self::new(requests, attempts, organizations, invites)
    }
}

impl UserRateLimiter {
    pub fn new(
        requests: RateLimits,
        attempts: RateLimits,
        organizations: RateLimits,
        invites: RateLimits,
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
            &bencher_otel::ApiCounter::UserAttemptMax,
            RateLimitingError::AuthEmail,
        );

        let RateLimits { minute, hour, day } = organizations;
        let organizations = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &bencher_otel::ApiCounter::UserOrganizationMax,
            RateLimitingError::AuthEmail,
        );

        let RateLimits { minute, hour, day } = invites;
        let invites = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &bencher_otel::ApiCounter::UserInviteMax,
            RateLimitingError::InviteEmail,
        );

        Self {
            requests,
            attempts,
            organizations,
            invites,
        }
    }

    pub fn check_request(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.requests.check(user_uuid)
    }

    pub fn check_auth(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.attempts.check(user_uuid)
    }

    pub fn check_organization(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.organizations.check(user_uuid)
    }

    pub fn check_invite(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.invites.check(user_uuid)
    }
}
