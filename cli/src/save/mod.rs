use tempfile::tempdir;

use crate::adapter::Report;
use crate::error::CliError;

mod clone;
mod git;

pub use git::Git;
