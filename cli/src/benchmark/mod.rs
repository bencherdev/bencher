use std::convert::TryFrom;
use std::process::Command;

use clap::Parser;

mod flag;
mod output;
mod shell;

use crate::adapter;
use crate::adapter::Adapter;
use crate::adapter::Report;
use crate::args::CliArgs;
use crate::args::CliBackend;
pub use crate::benchmark::flag::Flag;
pub use crate::benchmark::output::Output;
pub use crate::benchmark::shell::Shell;
use crate::error::CliError;
use crate::save::Git;

#[derive(Debug)]
pub struct Benchmark {
    shell: Shell,
    flag: Flag,
    cmd: String,
    adapter: Adapter,
    backend: Option<Backend>,
}

#[derive(Debug)]
pub enum Backend {
    Repo(Git),
}

impl TryFrom<CliArgs> for Benchmark {
    type Error = CliError;

    fn try_from(args: CliArgs) -> Result<Self, Self::Error> {
        Ok(Self {
            shell: Shell::try_from(args.shell)?,
            flag: Flag::try_from(args.flag)?,
            cmd: args.cmd,
            adapter: Adapter::from(args.adapter),
            backend: if let Some(backend) = args.backend {
                Some(Backend::try_from(backend)?)
            } else {
                None
            },
        })
    }
}

impl TryFrom<CliBackend> for Backend {
    type Error = CliError;

    fn try_from(backend: CliBackend) -> Result<Self, Self::Error> {
        Ok(match backend {
            CliBackend::Repo(repo) => Backend::Repo(Git::new(
                repo.url,
                repo.key,
                repo.name,
                repo.email,
                repo.message,
            )?),
        })
    }
}

impl Benchmark {
    pub fn new() -> Result<Self, CliError> {
        let args = CliArgs::parse();
        Self::try_from(args)
    }

    pub fn run(&self) -> Result<Output, CliError> {
        let output = Command::new(self.shell.to_string())
            .arg(self.flag.to_string())
            .arg(&self.cmd)
            .output()?;

        Output::try_from(output)
    }

    pub fn convert(&self, output: Output) -> Result<Report, CliError> {
        match &self.adapter {
            Adapter::Rust => adapter::rust::parse(output),
            Adapter::Custom(adapter) => adapter::custom::parse(&adapter, output),
        }
    }

    pub fn save(&self, report: Report) -> Result<(), CliError> {
        if let Some(backend) = &self.backend {
            match backend {
                Backend::Repo(git) => git.save(report),
            }
        } else {
            Ok(())
        }
    }
}
