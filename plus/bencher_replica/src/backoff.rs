//! Capped exponential backoff for storage errors.
//!
//! Pure state machine: it computes delays but never sleeps, so all timing
//! stays under the caller's injected clock.

use std::time::Duration;

/// Capped exponential backoff: `base, base*2, base*4, ..., cap`.
#[derive(Debug, Clone)]
pub struct Backoff {
    base: Duration,
    cap: Duration,
    current: Option<Duration>,
}

impl Backoff {
    /// Default base delay matching Litestream: 1 second.
    pub const DEFAULT_BASE: Duration = Duration::from_secs(1);
    /// Default delay cap matching Litestream: 5 minutes (300 seconds).
    pub const DEFAULT_CAP: Duration = Duration::from_mins(5);

    #[must_use]
    pub fn new(base: Duration, cap: Duration) -> Self {
        Self {
            base,
            cap,
            current: None,
        }
    }

    /// The next delay to wait before retrying: `base` on the first failure,
    /// doubling each subsequent call, saturating at `cap` (a cap below the
    /// base clamps the very first delay too).
    ///
    /// A `Duration::ZERO` base disables backoff entirely: doubling zero stays
    /// zero, so every retry is immediate and the cap never engages.
    pub fn next_delay(&mut self) -> Duration {
        let next = match self.current {
            None => self.base.min(self.cap),
            Some(current) => current.saturating_mul(2).min(self.cap),
        };
        self.current = Some(next);
        next
    }

    /// Reset after a success; the next failure starts back at `base`.
    pub fn reset(&mut self) {
        self.current = None;
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new(Self::DEFAULT_BASE, Self::DEFAULT_CAP)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use pretty_assertions::assert_eq;

    use super::Backoff;

    fn delays(backoff: &mut Backoff, count: usize) -> Vec<Duration> {
        std::iter::repeat_with(|| backoff.next_delay())
            .take(count)
            .collect()
    }

    #[test]
    fn next_delay_follows_exact_doubling_sequence() {
        let mut backoff = Backoff::new(Duration::from_secs(1), Duration::from_mins(5));
        let expected: Vec<Duration> = [1u64, 2, 4, 8, 16, 32, 64, 128, 256, 300, 300, 300]
            .into_iter()
            .map(Duration::from_secs)
            .collect();
        assert_eq!(delays(&mut backoff, 12), expected);
    }

    #[test]
    fn next_delay_saturates_at_cap() {
        let mut backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(4));
        let expected: Vec<Duration> = [1u64, 2, 4, 4, 4]
            .into_iter()
            .map(Duration::from_secs)
            .collect();
        assert_eq!(delays(&mut backoff, 5), expected);
    }

    #[test]
    fn reset_returns_to_base() {
        let mut backoff = Backoff::new(Duration::from_secs(1), Duration::from_mins(5));
        let _advanced = delays(&mut backoff, 4);
        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
        assert_eq!(backoff.next_delay(), Duration::from_secs(2));
    }

    #[test]
    fn defaults_match_litestream() {
        assert_eq!(Backoff::DEFAULT_BASE, Duration::from_secs(1));
        // The cap is 300 seconds, spelled in the unit clippy prefers.
        assert_eq!(Backoff::DEFAULT_CAP, Duration::from_mins(5));
        assert_eq!(Backoff::DEFAULT_CAP.as_secs(), 300);
        let mut backoff = Backoff::default();
        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
    }

    #[test]
    fn zero_base_yields_immediate_retries() {
        let mut backoff = Backoff::new(Duration::ZERO, Duration::from_mins(5));
        let expected = vec![Duration::ZERO; 6];
        assert_eq!(delays(&mut backoff, 6), expected);
        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::ZERO);
    }

    #[test]
    fn cap_below_base_clamps_first_delay() {
        let mut backoff = Backoff::new(Duration::from_secs(10), Duration::from_secs(3));
        let expected = vec![Duration::from_secs(3); 3];
        assert_eq!(delays(&mut backoff, 3), expected);
    }
}
