use std::fmt;

use bencher_json::RunnerResourceId;
use serde::Deserialize;

// A scrub disturbs its runner's Jobs, so no 2 runners scrub on the same or adjacent days.
const MIN_DAYS_APART: u8 = 2;

/// The day of the month of a runner's RAID scrub, at most 28 so that it comes every month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u8")]
pub struct ScrubDay(u8);

impl ScrubDay {
    const LAST: u8 = 28;

    /// Days between two scrub days, counting around the month.
    pub fn days_apart(self, other: Self) -> u8 {
        let days = self.0.abs_diff(other.0);
        days.min(Self::LAST - days)
    }
}

impl TryFrom<u8> for ScrubDay {
    type Error = anyhow::Error;

    fn try_from(day: u8) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..=Self::LAST).contains(&day),
            "scrub day must be from 1 to {}, got {day}",
            Self::LAST
        );
        Ok(Self(day))
    }
}

impl From<ScrubDay> for u8 {
    fn from(day: ScrubDay) -> Self {
        day.0
    }
}

impl fmt::Display for ScrubDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Every runner with no scrub day, and every pair of runners whose scrub days are too close.
pub fn scrub_day_problems(days: &[(RunnerResourceId, Option<ScrubDay>)]) -> Vec<String> {
    let mut problems: Vec<String> = days
        .iter()
        .filter(|(_, day)| day.is_none())
        .map(|(runner, _)| format!("{runner} has no scrub_day"))
        .collect();
    let set: Vec<(&RunnerResourceId, ScrubDay)> = days
        .iter()
        .filter_map(|(runner, day)| day.map(|day| (runner, day)))
        .collect();
    for (index, (runner, day)) in set.iter().enumerate() {
        for (other, other_day) in set.iter().skip(index + 1) {
            let apart = day.days_apart(*other_day);
            if apart < MIN_DAYS_APART {
                problems.push(format!(
                    "{runner} (day {day}) and {other} (day {other_day}) are {apart} day(s) apart"
                ));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(day: u8) -> ScrubDay {
        ScrubDay::try_from(day).unwrap()
    }

    fn runner(name: &str) -> RunnerResourceId {
        name.parse().unwrap()
    }

    #[test]
    fn scrub_day_is_a_day_every_month_has() {
        assert_eq!(serde_json::from_str::<ScrubDay>("1").unwrap(), day(1));
        assert_eq!(serde_json::from_str::<ScrubDay>("28").unwrap(), day(28));
        serde_json::from_str::<ScrubDay>("0").unwrap_err();
        serde_json::from_str::<ScrubDay>("29").unwrap_err();
    }

    #[test]
    fn days_apart_counts_around_the_month() {
        assert_eq!(day(5).days_apart(day(7)), 2);
        assert_eq!(day(7).days_apart(day(5)), 2);
        assert_eq!(day(28).days_apart(day(1)), 1);
        assert_eq!(day(27).days_apart(day(2)), 3);
        assert_eq!(day(1).days_apart(day(15)), 14);
    }

    #[test]
    fn scrub_days_two_days_apart_pass() {
        let days = [
            (runner("runner-a"), Some(day(1))),
            (runner("runner-b"), Some(day(3))),
            (runner("runner-c"), Some(day(27))),
        ];
        assert_eq!(scrub_day_problems(&days), Vec::<String>::new());
    }

    #[test]
    fn scrub_days_missing_or_too_close_fail() {
        let days = [
            (runner("runner-a"), Some(day(28))),
            (runner("runner-b"), Some(day(1))),
            (runner("runner-c"), None),
            (runner("runner-d"), Some(day(14))),
            (runner("runner-e"), Some(day(14))),
        ];
        assert_eq!(
            scrub_day_problems(&days),
            [
                "runner-c has no scrub_day",
                "runner-a (day 28) and runner-b (day 1) are 1 day(s) apart",
                "runner-d (day 14) and runner-e (day 14) are 0 day(s) apart",
            ]
        );
    }
}
