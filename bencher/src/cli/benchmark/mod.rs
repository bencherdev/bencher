use std::convert::TryFrom;
use std::process::Command;

use clap::Parser;

mod backend;
mod flag;
mod output;
mod shell;

use crate::cli::adapter;
use crate::cli::adapter::Adapter;
use crate::cli::adapter::Report;
use crate::cli::args::CliArgs;
use crate::error::CliError;
use backend::Backend;
pub use flag::Flag;
pub use output::Output;
pub use shell::Shell;

#[derive(Debug)]
pub struct Benchmark {
    shell: Shell,
    flag: Flag,
    cmd: String,
    adapter: Adapter,
    backend: Option<Backend>,
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
                Some(Backend::from(backend))
            } else {
                None
            },
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

    pub fn output(&self, report: Report) -> Result<(), CliError> {
        if let Some(backend) = &self.backend {
            match backend {
                Backend::Repo(git) => git.save(report),
            }
        } else {
            let report = serde_json::to_string(&report)?;
            println!("{report}");
            Ok(())
        }
    }
}
