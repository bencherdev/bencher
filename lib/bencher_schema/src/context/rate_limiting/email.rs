use bencher_json::{UserUuid, system::config::JsonEmailRateLimiter};

use crate::context::{
    RateLimitingError,
    rate_limiting::{RateLimiter, RateLimits},
};

const DEFAULT_ATTEMPTS_MINUTE_LIMIT: usize = 1 << 2;
const DEFAULT_ATTEMPTS_DAY_LIMIT: usize = 1 << 3;

const DEFAULT_INVITES_MINUTE_LIMIT: usize = 1 << 3;
const DEFAULT_INVITES_DAY_LIMIT: usize = 1 << 5;

pub(super) struct EmailRateLimiter {
    auth: RateLimiter<UserUuid>,
    invite: RateLimiter<UserUuid>,
}

impl Default for EmailRateLimiter {
    fn default() -> Self {
        let auth = RateLimits {
            minute_limit: DEFAULT_ATTEMPTS_MINUTE_LIMIT,
            day_limit: DEFAULT_ATTEMPTS_DAY_LIMIT,
        };

        let invite = RateLimits {
            minute_limit: DEFAULT_INVITES_MINUTE_LIMIT,
            day_limit: DEFAULT_INVITES_DAY_LIMIT,
        };

        Self::new(auth, invite)
    }
}

impl From<JsonEmailRateLimiter> for EmailRateLimiter {
    fn from(json: JsonEmailRateLimiter) -> Self {
        let minute_limit = json
            .auth
            .and_then(|r| r.minute_limit)
            .unwrap_or(DEFAULT_ATTEMPTS_MINUTE_LIMIT);
        let day_limit = json
            .auth
            .and_then(|r| r.day_limit)
            .unwrap_or(DEFAULT_ATTEMPTS_DAY_LIMIT);
        let auth = RateLimits {
            minute_limit,
            day_limit,
        };

        let minute_limit = json
            .invite
            .and_then(|r| r.minute_limit)
            .unwrap_or(DEFAULT_INVITES_MINUTE_LIMIT);
        let day_limit = json
            .invite
            .and_then(|r| r.day_limit)
            .unwrap_or(DEFAULT_INVITES_DAY_LIMIT);
        let invite = RateLimits {
            minute_limit,
            day_limit,
        };

        Self::new(auth, invite)
    }
}

impl EmailRateLimiter {
    pub fn new(auth: RateLimits, invite: RateLimits) -> Self {
        let RateLimits {
            minute_limit,
            day_limit,
        } = auth;
        let auth = RateLimiter::new(
            minute_limit,
            day_limit,
            #[cfg(feature = "otel")]
            &|interval| bencher_otel::ApiCounter::EmailMax(interval, bencher_otel::EmailKind::Auth),
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
            &|interval| {
                bencher_otel::ApiCounter::EmailMax(interval, bencher_otel::EmailKind::Invite)
            },
            RateLimitingError::InviteEmail,
        );

        Self { auth, invite }
    }

    pub fn check_auth(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.auth.check(user_uuid)
    }

    pub fn check_invite(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.invite.check(user_uuid)
    }
}
