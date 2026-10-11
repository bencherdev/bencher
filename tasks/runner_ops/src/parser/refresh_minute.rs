use std::fmt;

use bencher_json::RunnerResourceId;
use serde::Deserialize;

use super::spacing::spacing_problems;

// No 2 runners start their daily refreshes within 5 minutes of each other.
const MIN_MINUTES_APART: u8 = 5;

/// The minute past 05:00 UTC of a runner's daily refreshes, before the 06:00 upgrade timer; today, the apt package lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u8")]
pub struct RefreshMinute(u8);

impl RefreshMinute {
    const LAST: u8 = 59;

    /// Minutes between two refreshes, never counting around the hour: 05:59 and the next 05:00 are a day apart.
    pub fn minutes_apart(self, other: Self) -> u8 {
        self.0.abs_diff(other.0)
    }
}

impl TryFrom<u8> for RefreshMinute {
    type Error = anyhow::Error;

    fn try_from(minute: u8) -> anyhow::Result<Self> {
        anyhow::ensure!(
            minute <= Self::LAST,
            "refresh minute must be from 0 to {}, got {minute}",
            Self::LAST
        );
        Ok(Self(minute))
    }
}

impl From<RefreshMinute> for u8 {
    fn from(minute: RefreshMinute) -> Self {
        minute.0
    }
}

impl fmt::Display for RefreshMinute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Every runner with no refresh minute, and every pair of runners whose minutes are too close.
pub fn refresh_minute_problems(
    minutes: &[(RunnerResourceId, Option<RefreshMinute>)],
) -> Vec<String> {
    spacing_problems(
        minutes,
        "refresh_minute",
        "minute",
        MIN_MINUTES_APART,
        RefreshMinute::minutes_apart,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minute(minute: u8) -> RefreshMinute {
        RefreshMinute::try_from(minute).unwrap()
    }

    fn runner(name: &str) -> RunnerResourceId {
        name.parse().unwrap()
    }

    #[test]
    fn refresh_minute_is_a_minute_of_the_hour() {
        // Kills a range that drops minute 0 or admits 60, which would be 06:00.
        assert_eq!(
            serde_json::from_str::<RefreshMinute>("0").unwrap(),
            minute(0)
        );
        assert_eq!(
            serde_json::from_str::<RefreshMinute>("59").unwrap(),
            minute(59)
        );
        serde_json::from_str::<RefreshMinute>("60").unwrap_err();
        serde_json::from_str::<RefreshMinute>("-1").unwrap_err();
    }

    #[test]
    fn refresh_minutes_five_apart_pass_without_counting_around_the_hour() {
        // Kills a spacing that counts around the hour, which fails 0 and 59,
        // and one wider than 5 minutes.
        let minutes = [
            (runner("runner-a"), Some(minute(0))),
            (runner("runner-b"), Some(minute(5))),
            (runner("runner-c"), Some(minute(59))),
        ];
        assert_eq!(refresh_minute_problems(&minutes), Vec::<String>::new());
    }

    #[test]
    fn refresh_minutes_missing_or_too_close_fail() {
        // Kills a spacing narrower than 5 minutes, a missing minute that passes,
        // and a check of each runner against its neighbor alone.
        let minutes = [
            (runner("runner-a"), Some(minute(10))),
            (runner("runner-b"), Some(minute(30))),
            (runner("runner-c"), Some(minute(30))),
            (runner("runner-d"), None),
            (runner("runner-e"), Some(minute(14))),
        ];
        assert_eq!(
            refresh_minute_problems(&minutes),
            [
                "runner-d has no refresh_minute",
                "runner-a (minute 10) and runner-e (minute 14) are 4 minute(s) apart",
                "runner-b (minute 30) and runner-c (minute 30) are 0 minute(s) apart",
            ]
        );
    }
}
