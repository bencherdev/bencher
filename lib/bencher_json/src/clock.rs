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
}
