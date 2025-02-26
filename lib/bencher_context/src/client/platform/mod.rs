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
    Macos,
    Windows,
}

impl fmt::Display for OperatingSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                OperatingSystem::Linux => "Linux",
                OperatingSystem::Macos => "Macos",
                OperatingSystem::Windows => "Windows",
            }
        )
    }
}
