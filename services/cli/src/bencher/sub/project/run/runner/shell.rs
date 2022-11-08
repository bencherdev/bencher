use std::{convert::TryFrom, fmt};

use crate::CliError;

const UNIX_SHELL: &str = "/bin/sh";
const WINDOWS_SHELL: &str = "cmd";

#[derive(Debug)]
pub enum Shell {
    Unix,
    Windows,
    Custom(String),
}

impl TryFrom<Option<String>> for Shell {
    type Error = CliError;

    fn try_from(shell: Option<String>) -> Result<Self, Self::Error> {
        Ok(if let Some(shell) = shell {
            Self::Custom(shell)
        } else if cfg!(target_family = "unix") {
            Self::Unix
        } else if cfg!(target_family = "windows") {
            Self::Windows
        } else {
            return Err(CliError::Shell);
        })
    }
}

impl fmt::Display for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Unix => UNIX_SHELL,
                Self::Windows => WINDOWS_SHELL,
                Self::Custom(shell) => shell,
            }
        )
    }
}
