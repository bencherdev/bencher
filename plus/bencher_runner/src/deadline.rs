use std::time::{Duration, Instant};

/// A Job's timeout, which every iteration and every VM it boots draws from.
///
/// Kept as a start and a length rather than an `Instant`, which overflows on
/// the largest timeouts `runner run` accepts.
#[derive(Debug, Clone, Copy)]
pub struct JobDeadline {
    start: Instant,
    timeout: Duration,
}

impl JobDeadline {
    /// Starts the clock now.
    pub fn start(timeout: Duration) -> Self {
        Self {
            start: Instant::now(),
            timeout,
        }
    }

    pub fn started_at(&self) -> Instant {
        self.start
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Zero once the timeout has passed.
    pub fn remaining(&self) -> Duration {
        self.timeout.saturating_sub(self.start.elapsed())
    }

    /// The same start, `allowance` later.
    #[must_use]
    pub fn extended_by(&self, allowance: Duration) -> Self {
        Self {
            start: self.start,
            timeout: self.timeout.saturating_add(allowance),
        }
    }
}

/// What a test reads back from a timeout message.
#[cfg(test)]
pub(crate) mod timeout_message {
    use std::time::Duration;

    /// How much shorter than the time left just before a wait the time a
    /// message names may be, for the work between the two readings.
    const SLACK: Duration = Duration::from_millis(150);

    /// Asserts that `message` names, as `{:.1?}` prints it and just before
    /// "it had", a time no longer than `left`, read just before the wait, and
    /// at most [`SLACK`] shorter.
    pub(crate) fn assert_names_the_time_left(message: &str, left: Duration) {
        let had = message
            .split_once(" it had")
            .and_then(|(named, _)| named.rsplit_once(' '))
            .and_then(|(_, named)| debug_duration(named));
        // A millisecond over, for the rounding to one decimal place.
        assert!(
            had.is_some_and(|had| had <= left + Duration::from_millis(1) && had + SLACK >= left),
            "expected the {left:?} left before the wait, got: {message}"
        );
    }

    fn debug_duration(named: &str) -> Option<Duration> {
        [("ns", 1e-9), ("µs", 1e-6), ("ms", 1e-3), ("s", 1.0)]
            .into_iter()
            .find_map(|(unit, secs)| {
                let value: f64 = named.strip_suffix(unit)?.parse().ok()?;
                Some(Duration::from_secs_f64(value * secs))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_extended_deadline_keeps_its_start() {
        // Kills an extension that restarts the clock, which gives a VM booted
        // late in its Job the whole allowance past the Job's timeout and more.
        let deadline = JobDeadline::start(Duration::from_millis(300));
        std::thread::sleep(Duration::from_millis(200));

        let remaining = deadline.extended_by(Duration::from_secs(1)).remaining();

        assert!(
            remaining <= Duration::from_millis(1100),
            "remaining {remaining:?}"
        );
    }

    #[test]
    fn the_largest_timeout_extends_without_overflow() {
        // Kills an addition that panics or wraps for `runner run`'s largest timeouts.
        let deadline = JobDeadline::start(Duration::MAX).extended_by(Duration::from_secs(5));

        assert_eq!(deadline.timeout(), Duration::MAX);
    }
}
