use crate::error::{ConfigError, RunnerError};

/// Validated swappiness value (0–200).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swappiness(u32);

impl Swappiness {
    pub const MAX: u32 = 200;

    /// Default swappiness for benchmarks (10).
    pub const DEFAULT: Self = Self(10);
}

impl TryFrom<u32> for Swappiness {
    type Error = RunnerError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value > Self::MAX {
            return Err(ConfigError::OutOfRange {
                name: "swappiness",
                value: value.to_string(),
                range: "0–200",
            }
            .into());
        }
        Ok(Self(value))
    }
}

impl std::fmt::Display for Swappiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_zero() {
        let s = Swappiness::try_from(0).unwrap();
        assert_eq!(s.to_string(), "0");
    }

    #[test]
    fn valid_max() {
        let s = Swappiness::try_from(200).unwrap();
        assert_eq!(s.to_string(), "200");
    }

    #[test]
    fn valid_default() {
        let s = Swappiness::try_from(10).unwrap();
        assert_eq!(s.to_string(), "10");
    }

    #[test]
    fn invalid_over_max() {
        let err = Swappiness::try_from(201).unwrap_err();
        assert!(err.to_string().contains("out of range"));
    }
}
