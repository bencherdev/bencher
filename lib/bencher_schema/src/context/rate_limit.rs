use std::time::Duration;

use bencher_json::{system::config::JsonRateLimit, DateTime};

const DAY: Duration = Duration::from_secs(24 * 60 * 60);
const UNCLAIMED_RATE_LIMIT: u32 = u8::MAX as u32;
const CLAIMED_RATE_LIMIT: u32 = u16::MAX as u32;

pub struct RateLimit {
    pub window: Duration,
    pub unclaimed: u32,
    pub claimed: u32,
}

impl From<JsonRateLimit> for RateLimit {
    fn from(json: JsonRateLimit) -> Self {
        let JsonRateLimit {
            window,
            unclaimed,
            claimed,
        } = json;
        Self {
            window: window.map(u64::from).map_or(DAY, Duration::from_secs),
            unclaimed: unclaimed.unwrap_or(UNCLAIMED_RATE_LIMIT),
            claimed: claimed.unwrap_or(CLAIMED_RATE_LIMIT),
        }
    }
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            window: DAY,
            unclaimed: UNCLAIMED_RATE_LIMIT,
            claimed: CLAIMED_RATE_LIMIT,
        }
    }
}

impl RateLimit {
    pub fn window(&self) -> (DateTime, DateTime) {
        let end_time = chrono::Utc::now();
        let start_time = end_time - self.window;
        (start_time.into(), end_time.into())
    }
}
