use http::{HeaderMap, StatusCode, header::RETRY_AFTER};

const X_RATELIMIT_REMAINING: &str = "x-ratelimit-remaining";
const X_RATELIMIT_RESET: &str = "x-ratelimit-reset";

/// What a 403 or a 429 says about the receiver's rate limit. Any other answer reads none of it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RateLimit {
    /// `retry-after` in seconds; an HTTP-date counts as absent
    pub retry_after: Option<u64>,
    /// `x-ratelimit-remaining`
    pub remaining: Option<u64>,
    /// `x-ratelimit-reset`, in Unix seconds
    pub reset: Option<i64>,
}

impl RateLimit {
    pub(super) fn read(status: StatusCode, headers: &HeaderMap) -> Self {
        if status != StatusCode::FORBIDDEN && status != StatusCode::TOO_MANY_REQUESTS {
            return Self::default();
        }
        let number = |name| {
            let value = headers.get(name)?.to_str().ok()?;
            // Digits alone, so only an overflow fails to parse, and it saturates.
            (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| value.parse().unwrap_or(u64::MAX))
        };
        Self {
            retry_after: number(RETRY_AFTER.as_str()),
            remaining: number(X_RATELIMIT_REMAINING),
            reset: number(X_RATELIMIT_RESET).map(|reset| i64::try_from(reset).unwrap_or(i64::MAX)),
        }
    }
}
