use bencher_valid::DateTime;

/// Injectable clock for deterministic time in tests.
/// Reused by both OCI storage and the runner system.
#[derive(Clone)]
pub enum Clock {
    /// Real system clock — `DateTime::now()`.
    System,
    /// Custom time source (test-only).
    #[cfg(any(test, feature = "test-clock"))]
    Custom(std::sync::Arc<dyn Fn() -> DateTime + Send + Sync>),
}

impl Clock {
    /// Returns the current `DateTime`.
    pub fn now(&self) -> DateTime {
        match self {
            Self::System => DateTime::now(),
            #[cfg(any(test, feature = "test-clock"))]
            Self::Custom(f) => f(),
        }
    }

    /// Convenience: returns the current Unix timestamp in seconds.
    pub fn timestamp(&self) -> i64 {
        self.now().timestamp()
    }

    /// Returns `(app_timestamp, os_timestamp)`.
    ///
    /// - **`app_timestamp`** — from the (possibly mocked) application clock.
    ///   Compare against values written through [`Self::timestamp`]
    ///   (e.g. `state.created_at`).
    /// - **`os_timestamp`** — real OS wall-clock time.  Compare against
    ///   OS-level timestamps (filesystem mtime, S3 `LastModified`, …)
    ///   that are outside the application's control.
    ///
    /// In production (`Clock::System`) both values come from a single
    /// `SystemTime::now()` call and are identical.
    pub fn timestamps(&self) -> (i64, i64) {
        let os_now = Self::system_timestamp();
        match self {
            Self::System => (os_now, os_now),
            #[cfg(any(test, feature = "test-clock"))]
            Self::Custom(f) => (f().timestamp(), os_now),
        }
    }

    fn system_timestamp() -> i64 {
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        )
        .unwrap_or(i64::MAX)
    }
}
