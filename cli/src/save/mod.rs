use tempfile::tempdir;

use crate::adapter::Report;
use crate::error::CliError;

mod git;

pub use git::Git;
