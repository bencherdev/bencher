/// Error type for invalid firecracker log level strings.
#[derive(Debug, thiserror::Error)]
#[error("Invalid firecracker log level: {0} (expected: off, error, warning, info, debug, trace)")]
pub struct ParseLogLevelError(pub String);

/// Firecracker log level (maps to `--level` CLI flag).
#[derive(Debug, Clone, Copy, Default)]
pub enum FirecrackerLogLevel {
    Off,
    Error,
    #[default]
    Warning,
    Info,
    Debug,
    Trace,
}

impl FirecrackerLogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Error => "Error",
            Self::Warning => "Warning",
            Self::Info => "Info",
            Self::Debug => "Debug",
            Self::Trace => "Trace",
        }
    }
}

impl std::fmt::Display for FirecrackerLogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for FirecrackerLogLevel {
    type Err = ParseLogLevelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "error" => Ok(Self::Error),
            "warning" => Ok(Self::Warning),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            "trace" => Ok(Self::Trace),
            _ => Err(ParseLogLevelError(s.to_owned())),
        }
    }
}
