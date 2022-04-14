use std::convert::TryFrom;
use std::fmt;

use git2::Repository;

use crate::adapter::Report;
use crate::error::CliError;
use crate::save::clone::clone;

pub struct Git {
    source: String,
    // repo: Repository,
}

impl TryFrom<String> for Git {
    type Error = CliError;

    fn try_from(source: String) -> Result<Self, Self::Error> {
        Ok(Self { source })
    }
}

impl fmt::Debug for Git {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Git").field("source", &self.source).finish()
    }
}

impl Git {
    pub fn save(&self, report: Report) -> Result<(), CliError> {
        clone();
        Ok(())
    }
}
