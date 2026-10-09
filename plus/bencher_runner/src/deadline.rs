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
}
