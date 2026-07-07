//! Transparent hugepage (THP) mode for the host.
//!
//! THP collapses regular pages into huge pages in the background, so the
//! memory backing of a benchmark process differs nondeterministically
//! between runs. Pinning the mode to `never` makes backing deterministic;
//! `leave` preserves whatever the host has configured.

/// Error type for invalid THP mode strings.
#[derive(Debug, thiserror::Error)]
#[error("Invalid transparent hugepage mode: {0} (expected: never, madvise, always, leave)")]
pub struct ParseThpModeError(pub String);

/// Host transparent hugepage mode applied to
/// `/sys/kernel/mm/transparent_hugepage/{enabled,defrag}`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ThpMode {
    /// Disable THP for deterministic memory backing (the default).
    #[default]
    Never,
    /// THP only for regions that request it via `madvise`.
    Madvise,
    /// THP for all anonymous memory.
    Always,
    /// Do not touch the host THP configuration.
    Leave,
}

impl ThpMode {
    /// The value to write to the sysfs files, or `None` for [`Self::Leave`].
    #[must_use]
    pub fn sysfs_value(self) -> Option<&'static str> {
        match self {
            Self::Never => Some("never"),
            Self::Madvise => Some("madvise"),
            Self::Always => Some("always"),
            Self::Leave => None,
        }
    }
}

impl std::fmt::Display for ThpMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.sysfs_value().unwrap_or("leave"))
    }
}

impl std::str::FromStr for ThpMode {
    type Err = ParseThpModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "never" => Ok(Self::Never),
            "madvise" => Ok(Self::Madvise),
            "always" => Ok(Self::Always),
            "leave" => Ok(Self::Leave),
            _ => Err(ParseThpModeError(s.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_never() {
        assert_eq!(ThpMode::default(), ThpMode::Never);
    }

    #[test]
    fn from_str_round_trip() {
        for mode in [
            ThpMode::Never,
            ThpMode::Madvise,
            ThpMode::Always,
            ThpMode::Leave,
        ] {
            assert_eq!(mode.to_string().parse::<ThpMode>().unwrap(), mode);
        }
    }

    #[test]
    fn from_str_case_insensitive() {
        assert_eq!("NEVER".parse::<ThpMode>().unwrap(), ThpMode::Never);
        assert_eq!("Leave".parse::<ThpMode>().unwrap(), ThpMode::Leave);
    }

    #[test]
    fn from_str_invalid() {
        let err = "sometimes".parse::<ThpMode>().unwrap_err();
        assert!(err.to_string().contains("sometimes"));
    }

    #[test]
    fn sysfs_values() {
        assert_eq!(ThpMode::Never.sysfs_value(), Some("never"));
        assert_eq!(ThpMode::Madvise.sysfs_value(), Some("madvise"));
        assert_eq!(ThpMode::Always.sysfs_value(), Some("always"));
        assert_eq!(ThpMode::Leave.sysfs_value(), None);
    }
}
