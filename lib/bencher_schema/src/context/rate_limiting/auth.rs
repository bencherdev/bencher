use bencher_json::{UserUuid, system::config::JsonAuthRateLimiter};

use crate::context::{
    RateLimitingError,
    rate_limiting::{RateLimiter, RateLimits},
};

const DEFAULT_ATTEMPT_MINUTE_LIMIT: usize = 1 << 2;
const DEFAULT_ATTEMPT_DAY_LIMIT: usize = 1 << 3;

const DEFAULT_INVITE_MINUTE_LIMIT: usize = 1 << 3;
const DEFAULT_INVITE_DAY_LIMIT: usize = 1 << 5;

pub(super) struct AuthRateLimiter {
    attempt: RateLimiter<UserUuid>,
    invite: RateLimiter<UserUuid>,
}

impl Default for AuthRateLimiter {
    fn default() -> Self {
        let attempt = RateLimits {
            minute_limit: DEFAULT_ATTEMPT_MINUTE_LIMIT,
            day_limit: DEFAULT_ATTEMPT_DAY_LIMIT,
        };

        let invite = RateLimits {
            minute_limit: DEFAULT_INVITE_MINUTE_LIMIT,
            day_limit: DEFAULT_INVITE_DAY_LIMIT,
        };

        Self::new(attempt, invite)
    }
}

impl From<JsonAuthRateLimiter> for AuthRateLimiter {
    fn from(json: JsonAuthRateLimiter) -> Self {
        let minute_limit = json
            .attempt
            .and_then(|r| r.minute_limit)
            .unwrap_or(DEFAULT_ATTEMPT_MINUTE_LIMIT);
        let day_limit = json
            .attempt
            .and_then(|r| r.day_limit)
            .unwrap_or(DEFAULT_ATTEMPT_DAY_LIMIT);
        let attempt = RateLimits {
            minute_limit,
            day_limit,
        };

        let minute_limit = json
            .invite
            .and_then(|r| r.minute_limit)
            .unwrap_or(DEFAULT_INVITE_MINUTE_LIMIT);
        let day_limit = json
            .invite
            .and_then(|r| r.day_limit)
            .unwrap_or(DEFAULT_INVITE_DAY_LIMIT);
        let invite = RateLimits {
            minute_limit,
            day_limit,
        };

        Self::new(attempt, invite)
    }
}

impl AuthRateLimiter {
    pub fn new(attempt: RateLimits, invite: RateLimits) -> Self {
        let RateLimits {
            minute_limit,
            day_limit,
        } = attempt;
        let attempt = RateLimiter::new(
            minute_limit,
            day_limit,
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::AuthMax(interval, bencher_otel::AuthKind::Attempt)
            },
            RateLimitingError::AuthEmail,
        );

        let RateLimits {
            minute_limit,
            day_limit,
        } = invite;
        let invite = RateLimiter::new(
            minute_limit,
            day_limit,
            #[cfg(feature = "otel")]
            &|interval| bencher_otel::ApiCounter::AuthMax(interval, bencher_otel::AuthKind::Invite),
            RateLimitingError::InviteEmail,
        );

        Self { attempt, invite }
    }

    pub fn check_auth(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.attempt.check(user_uuid)
    }

    pub fn check_invite(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.invite.check(user_uuid)
    }
}
