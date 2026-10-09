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
