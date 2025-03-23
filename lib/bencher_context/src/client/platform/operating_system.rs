use std::fmt;

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

impl OperatingSystem {
    pub fn current() -> Option<Self> {
        if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else if cfg!(target_os = "macos") {
            Some(Self::MacOS)
        } else if cfg!(target_os = "windows") {
            Some(Self::Windows)
        } else {
            None
        }
    }
}
