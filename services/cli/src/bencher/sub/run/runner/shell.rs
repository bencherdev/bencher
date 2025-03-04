use std::fmt;

use crate::bencher::sub::RunError;

const UNIX_SHELL: &str = "/bin/sh";
const WINDOWS_SHELL: &str = "cmd";

#[derive(Debug, Clone)]
pub enum Shell {
    Unix,
    Windows,
    Custom(String),
}

impl TryFrom<Option<String>> for Shell {
    type Error = RunError;

    fn try_from(shell: Option<String>) -> Result<Self, Self::Error> {
        Ok(if let Some(shell) = shell {
            Self::Custom(shell)
        } else if cfg!(target_family = "unix") {
            Self::Unix
        } else if cfg!(target_family = "windows") {
            Self::Windows
        } else {
            return Err(RunError::Shell);
        })
    }
}

impl AsRef<str> for Shell {
    fn as_ref(&self) -> &str {
        match self {
            Self::Unix => UNIX_SHELL,
            Self::Windows => WINDOWS_SHELL,
            Self::Custom(shell) => shell,
        }
    }
}

impl fmt::Display for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}
