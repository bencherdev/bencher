use std::sync::LazyLock;

use bencher_json::system::config::JsonStats;
use chrono::NaiveTime;

// Run at 03:07:22 UTC by default (offset 11,242 seconds)
#[expect(clippy::expect_used)]
static DEFAULT_STATS_OFFSET: LazyLock<NaiveTime> =
    LazyLock::new(|| NaiveTime::from_hms_opt(3, 7, 22).expect("Invalid default stats offset"));
// Default stats to enabled
const DEFAULT_STATS_ENABLED: bool = true;

#[derive(Debug, Clone, Copy)]
pub struct StatsSettings {
    pub offset: NaiveTime,
    pub enabled: bool,
}

impl Default for StatsSettings {
    fn default() -> Self {
        Self {
            offset: *DEFAULT_STATS_OFFSET,
            enabled: DEFAULT_STATS_ENABLED,
        }
    }
}

impl From<JsonStats> for StatsSettings {
    fn from(json: JsonStats) -> Self {
        let JsonStats { offset, enabled } = json;
        let offset = offset
            .and_then(|offset| NaiveTime::from_num_seconds_from_midnight_opt(offset, 0))
            .unwrap_or(*DEFAULT_STATS_OFFSET);
        let enabled = enabled.unwrap_or(DEFAULT_STATS_ENABLED);
        Self { offset, enabled }
    }
}
