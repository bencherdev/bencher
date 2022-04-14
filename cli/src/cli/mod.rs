use std::convert::TryFrom;
use std::process::Command;

mod flag;
mod shell;

use crate::adapter::Adapter;
use crate::args::Args;
use crate::cli::flag::Flag;
use crate::cli::shell::Shell;
use crate::error::CliError;

#[derive(Debug)]
pub struct Cli {
    shell: Shell,
    flag: Flag,
    cmd: String,
    adapter: Adapter,
    tag: Option<Vec<String>>,
}

impl TryFrom<Args> for Cli {
    type Error = CliError;

    fn try_from(args: Args) -> Result<Self, Self::Error> {
        Ok(Self {
            shell: Shell::try_from(args.shell)?,
            flag: Flag::try_from(args.flag)?,
            cmd: args.cmd,
            adapter: Adapter::from(args.adapter),
            tag: args.tag,
        })
    }
}
