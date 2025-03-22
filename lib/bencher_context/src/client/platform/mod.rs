use std::fmt;

use uuid::Uuid;

mod target_os;

#[derive(Debug, Clone, Copy)]
pub struct Fingerprint(Uuid);

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum OperatingSystem {
    Linux,
    MacOS,
    Windows,
}

impl fmt::Display for OperatingSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                OperatingSystem::Linux => "Linux",
                OperatingSystem::MacOS => "macOS",
                OperatingSystem::Windows => "Windows",
            }
        )
    }
}

#[cfg(all(
    not(target_os = "linux"),
    not(target_os = "macos"),
    not(target_os = "windows")
))]
impl OperatingSystem {
    #[allow(clippy::unnecessary_wraps)]
    pub fn current() -> Option<Self> {
        None
    }
}
