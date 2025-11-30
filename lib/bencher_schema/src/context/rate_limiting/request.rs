use std::net::IpAddr;

use bencher_json::{UserUuid, system::config::JsonRequestRateLimiter};

use crate::context::{
    RateLimitingError,
    rate_limiting::{RateLimiter, RateLimits},
};

const DEFAULT_PUBLIC_MINUTE_LIMIT: usize = 1 << 10;
const DEFAULT_PUBLIC_HOUR_LIMIT: usize = 1 << 12;
const DEFAULT_PUBLIC_DAY_LIMIT: usize = 1 << 13;

const DEFAULT_USER_MINUTE_LIMIT: usize = 1 << 11;
const DEFAULT_USER_HOUR_LIMIT: usize = 1 << 13;
const DEFAULT_USER_DAY_LIMIT: usize = 1 << 14;

pub(super) struct RequestRateLimiter {
    public: RateLimiter<IpAddr>,
    user: RateLimiter<UserUuid>,
}

impl Default for RequestRateLimiter {
    fn default() -> Self {
        let public = RateLimits {
            minute: DEFAULT_PUBLIC_MINUTE_LIMIT,
            hour: DEFAULT_PUBLIC_HOUR_LIMIT,
            day: DEFAULT_PUBLIC_DAY_LIMIT,
        };

        let user = RateLimits {
            minute: DEFAULT_USER_MINUTE_LIMIT,
            hour: DEFAULT_USER_HOUR_LIMIT,
            day: DEFAULT_USER_DAY_LIMIT,
        };

        Self::new(public, user)
    }
}

impl From<JsonRequestRateLimiter> for RequestRateLimiter {
    fn from(json: JsonRequestRateLimiter) -> Self {
        let minute = json
            .public
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_PUBLIC_MINUTE_LIMIT);
        let hour = json
            .public
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_PUBLIC_HOUR_LIMIT);
        let day = json
            .public
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_PUBLIC_DAY_LIMIT);
        let public = RateLimits { minute, hour, day };

        let minute = json
            .user
            .and_then(|r| r.minute)
            .unwrap_or(DEFAULT_USER_MINUTE_LIMIT);
        let hour = json
            .user
            .and_then(|r| r.hour)
            .unwrap_or(DEFAULT_USER_HOUR_LIMIT);
        let day = json
            .user
            .and_then(|r| r.day)
            .unwrap_or(DEFAULT_USER_DAY_LIMIT);
        let user = RateLimits { minute, hour, day };

        Self::new(public, user)
    }
}

impl RequestRateLimiter {
    pub fn new(public: RateLimits, user: RateLimits) -> Self {
        let RateLimits { minute, hour, day } = public;
        let public = RateLimiter::new(
            minute,
            hour,
            day,
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::RequestMax(
                    interval,
                    bencher_otel::AuthorizationKind::Public,
                )
            },
            RateLimitingError::IpAddressRequests,
        );

        let RateLimits { minute, hour, day } = user;
        let user = RateLimiter::new(
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

        Self { public, user }
    }

    pub fn check_public(&self, ip: IpAddr) -> Result<(), dropshot::HttpError> {
        self.public.check(ip)
    }

    pub fn check_user(&self, user_uuid: UserUuid) -> Result<(), dropshot::HttpError> {
        self.user.check(user_uuid)
    }
}
