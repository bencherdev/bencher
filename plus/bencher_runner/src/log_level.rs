/// Error type for invalid sandbox log level strings.
#[derive(Debug, thiserror::Error)]
#[error("Invalid sandbox log level: {0} (expected: off, error, warning, info, debug, trace)")]
pub struct ParseLogLevelError(pub String);

/// Sandbox process log level (maps to `--level` CLI flag for Firecracker).
#[derive(Debug, Clone, Copy, Default)]
pub enum SandboxLogLevel {
    Off,
    Error,
    #[default]
    Warning,
    Info,
    Debug,
    Trace,
}

impl SandboxLogLevel {
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

impl std::fmt::Display for SandboxLogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SandboxLogLevel {
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
