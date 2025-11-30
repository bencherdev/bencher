use bencher_json::{UserUuid, system::config::JsonAuthRateLimiter};

use crate::context::{
    RateLimitingError,
    rate_limiting::{RateLimiter, RateLimits},
};

const DEFAULT_ATTEMPT_MINUTE_LIMIT: usize = 1 << 1;
const DEFAULT_ATTEMPT_HOUR_LIMIT: usize = 1 << 2;
const DEFAULT_ATTEMPT_DAY_LIMIT: usize = 1 << 3;

const DEFAULT_INVITE_MINUTE_LIMIT: usize = 1 << 3;
const DEFAULT_INVITE_HOUR_LIMIT: usize = 1 << 4;
const DEFAULT_INVITE_DAY_LIMIT: usize = 1 << 5;

pub(super) struct AuthRateLimiter {
    attempt: RateLimiter<UserUuid>,
    invite: RateLimiter<UserUuid>,
}

impl Default for AuthRateLimiter {
    fn default() -> Self {
        let attempt = RateLimits {
            minute: DEFAULT_ATTEMPT_MINUTE_LIMIT,
            hour: DEFAULT_ATTEMPT_HOUR_LIMIT,
            day: DEFAULT_ATTEMPT_DAY_LIMIT,
        };

        let invite = RateLimits {
            minute: DEFAULT_INVITE_MINUTE_LIMIT,
            hour: DEFAULT_INVITE_HOUR_LIMIT,
            day: DEFAULT_INVITE_DAY_LIMIT,
        };

        Self::new(attempt, invite)
    }
}

impl From<JsonAuthRateLimiter> for AuthRateLimiter {
    fn from(json: JsonAuthRateLimiter) -> Self {
        let minute = json
            .attempt
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_ATTEMPT_MINUTE_LIMIT);
        let hour = json
            .attempt
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_ATTEMPT_HOUR_LIMIT);
        let day = json
            .attempt
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_ATTEMPT_DAY_LIMIT);
        let attempt = RateLimits { minute, hour, day };

        let minute = json
            .invite
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_INVITE_MINUTE_LIMIT);
        let hour = json
            .invite
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_INVITE_HOUR_LIMIT);
        let day = json
            .invite
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_INVITE_DAY_LIMIT);
        let invite = RateLimits { minute, hour, day };

        Self::new(attempt, invite)
    }
}

impl AuthRateLimiter {
    pub fn new(attempt: RateLimits, invite: RateLimits) -> Self {
        let RateLimits { minute, hour, day } = attempt;
        let attempt = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::AuthMax(interval, bencher_otel::AuthKind::Attempt)
            },
            RateLimitingError::AuthEmail,
        );

        let RateLimits { minute, hour, day } = invite;
        let invite = RateLimiter::new(
            minute,
            hour,
            day,
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
