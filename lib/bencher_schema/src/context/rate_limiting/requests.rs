use std::net::IpAddr;

use bencher_json::UserUuid;

use crate::context::{RateLimitingError, rate_limiting::RateLimiter};

pub(super) struct RequestsRateLimiter {
    public: RateLimiter<IpAddr>,
    user: RateLimiter<UserUuid>,
}

impl RequestsRateLimiter {
    pub fn new() -> Self {
        let public = RateLimiter::new(
            60,   // 60 requests per minute
            1000, // 1000 requests per day
            #[cfg(feature = "otel")]
            &|interval| {
                bencher_otel::ApiCounter::RequestMax(
                    interval,
                    bencher_otel::AuthorizationKind::Public,
                )
            },
            RateLimitingError::IpAddressRequests,
        );

        let user = RateLimiter::new(
            120,  // 120 requests per minute
            5000, // 5000 requests per day
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
}
