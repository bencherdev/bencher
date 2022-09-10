use std::{convert::TryFrom, fmt};

use crate::CliError;

const UNIX_FLAG: &str = "-c";
const WINDOWS_FLAG: &str = "/C";

#[derive(Debug)]
pub enum Flag {
    Unix,
    Windows,
    Custom(String),
}

impl TryFrom<Option<String>> for Flag {
    type Error = CliError;

    fn try_from(shell: Option<String>) -> Result<Self, Self::Error> {
        Ok(if let Some(shell) = shell {
            Self::Custom(shell)
        } else if cfg!(target_family = "unix") {
            Self::Unix
        } else if cfg!(target_family = "windows") {
            Self::Windows
        } else {
            return Err(CliError::Flag);
        })
    }
}

impl fmt::Display for Flag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Unix => UNIX_FLAG,
                Self::Windows => WINDOWS_FLAG,
                Self::Custom(shell) => shell,
            }
        )
    }
}
