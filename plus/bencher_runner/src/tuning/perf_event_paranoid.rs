use crate::error::{ConfigError, RunnerError};

/// Validated `perf_event_paranoid` value (-1 to 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerfEventParanoid(i32);

impl PerfEventParanoid {
    pub const MIN: i32 = -1;
    pub const MAX: i32 = 4;

    /// Default `perf_event_paranoid` for benchmarks (-1).
    pub const DEFAULT: Self = Self(-1);
}

impl TryFrom<i32> for PerfEventParanoid {
    type Error = RunnerError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(ConfigError::OutOfRange {
                name: "perf_event_paranoid",
                value: value.to_string(),
                range: "-1 to 4",
            }
            .into());
        }
        Ok(Self(value))
    }
}

impl std::fmt::Display for PerfEventParanoid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_min() {
        let p = PerfEventParanoid::try_from(-1).unwrap();
        assert_eq!(p.to_string(), "-1");
    }

    #[test]
    fn valid_max() {
        let p = PerfEventParanoid::try_from(4).unwrap();
        assert_eq!(p.to_string(), "4");
    }

    #[test]
    fn valid_zero() {
        let p = PerfEventParanoid::try_from(0).unwrap();
        assert_eq!(p.to_string(), "0");
    }

    #[test]
    fn invalid_below_min() {
        let err = PerfEventParanoid::try_from(-2).unwrap_err();
        assert!(err.to_string().contains("out of range"));
    }

    #[test]
    fn invalid_above_max() {
        let err = PerfEventParanoid::try_from(5).unwrap_err();
        assert!(err.to_string().contains("out of range"));
    }
}
