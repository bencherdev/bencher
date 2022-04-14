use clap::Parser;

use crate::adapter::Adapter;
use crate::error::CliError;

const UNIX_SHELL: &str = "/bin/sh";
const WINDOWS_SHELL: &str = "cmd";

const UNIX_FLAG: &str = "-c";
const WINDOWS_FLAG: &str = "/C";

/// Time Series Benchmarking
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Shell command path
    #[clap(short, long)]
    pub shell: Option<String>,

    /// Shell command flag
    #[clap(short, long)]
    pub flag: Option<String>,

    /// Benchmark command to execute
    #[clap(short = 'x', long = "exec")]
    pub cmd: String,

    /// Benchmark output adapter
    #[clap(short, long, default_value = "rust")]
    pub adapter: String,

    /// Output tags
    #[clap(short, long)]
    pub tag: Option<Vec<String>>,
}

impl Args {
    pub fn shell(&self) -> Result<String, CliError> {
        Ok(if let Some(shell) = self.shell.clone() {
            shell
        } else if cfg!(target_family = "unix") {
            UNIX_SHELL.into()
        } else if cfg!(target_family = "windows") {
            WINDOWS_SHELL.into()
        } else {
            return Err(CliError::Shell);
        })
    }

    pub fn flag(&self) -> Result<String, CliError> {
        Ok(if let Some(flag) = self.flag.clone() {
            flag
        } else if cfg!(target_family = "unix") {
            UNIX_FLAG.into()
        } else if cfg!(target_family = "windows") {
            WINDOWS_FLAG.into()
        } else {
            return Err(CliError::Flag);
        })
    }

    pub fn cmd(&self) -> &str {
        &self.cmd
    }

    pub fn adapter(&self) -> Adapter {
        self.adapter.clone().into()
    }
}
