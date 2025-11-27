use std::net::IpAddr;

use bencher_json::{UserUuid, system::config::JsonRequestRateLimiter};

use crate::context::{
    RateLimitingError,
    rate_limiting::{RateLimiter, RateLimits},
};

const DEFAULT_PUBLIC_MINUTE_LIMIT: usize = 1 << 10;
const DEFAULT_PUBLIC_DAY_LIMIT: usize = 1 << 13;

const DEFAULT_USER_MINUTE_LIMIT: usize = 1 << 11;
const DEFAULT_USER_DAY_LIMIT: usize = 1 << 14;

pub(super) struct RequestRateLimiter {
    public: RateLimiter<IpAddr>,
    user: RateLimiter<UserUuid>,
}

impl Default for RequestRateLimiter {
    fn default() -> Self {
        let public = RateLimits {
            minute_limit: DEFAULT_PUBLIC_MINUTE_LIMIT,
            day_limit: DEFAULT_PUBLIC_DAY_LIMIT,
        };

        let user = RateLimits {
            minute_limit: DEFAULT_USER_MINUTE_LIMIT,
            day_limit: DEFAULT_USER_DAY_LIMIT,
        };

        Self::new(public, user)
    }
}

impl From<JsonRequestRateLimiter> for RequestRateLimiter {
    fn from(json: JsonRequestRateLimiter) -> Self {
        let minute_limit = json
            .public
            .and_then(|r| r.minute_limit)
            .unwrap_or(DEFAULT_PUBLIC_MINUTE_LIMIT);
        let day_limit = json
            .public
            .and_then(|r| r.day_limit)
            .unwrap_or(DEFAULT_PUBLIC_DAY_LIMIT);
        let public = RateLimits {
            minute_limit,
            day_limit,
        };

        let minute_limit = json
            .user
            .and_then(|r| r.minute_limit)
            .unwrap_or(DEFAULT_USER_MINUTE_LIMIT);
        let day_limit = json
            .user
            .and_then(|r| r.day_limit)
            .unwrap_or(DEFAULT_USER_DAY_LIMIT);
        let user = RateLimits {
            minute_limit,
            day_limit,
        };

        Self::new(public, user)
    }
}

impl RequestRateLimiter {
    pub fn new(public: RateLimits, user: RateLimits) -> Self {
        let RateLimits {
            minute_limit,
            day_limit,
        } = public;
        let public = RateLimiter::new(
            minute_limit,
            day_limit,
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::RequestMax(
                    interval,
                    bencher_otel::AuthorizationKind::Public,
                )
            },
            RateLimitingError::IpAddressRequests,
        );

        let RateLimits {
            minute_limit,
            day_limit,
        } = user;
        let user = RateLimiter::new(
            minute_limit,
            day_limit,
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::RequestMax(
                    interval,
                    bencher_otel::AuthorizationKind::User,
                )
            },
            RateLimitingError::UserRequests,
        );

        Self { public, user }
    }

    pub fn check_public(&self, ip: IpAddr) -> Result<(), dropshot::HttpError> {
        self.public.check(ip)
    }

    pub fn check_user(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.user.check(user_uuid)
    }
}
