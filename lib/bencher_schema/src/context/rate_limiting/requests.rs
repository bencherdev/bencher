use std::net::IpAddr;

use bencher_json::UserUuid;

use super::RateLimiter;

pub(super) struct RequestsRateLimiter {
    public: RateLimiter<IpAddr>,
    user: RateLimiter<UserUuid>,
}
