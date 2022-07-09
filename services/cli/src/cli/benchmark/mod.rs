use std::convert::{
    TryFrom,
    TryInto,
};

use chrono::Utc;

use crate::{
    cli::clap::CliCommand,
    BencherError,
};

mod command;
mod flag;
mod input;
mod output;
mod shell;

use command::Command;
use input::Input;
pub use output::Output;

#[derive(Debug)]
pub enum Benchmark {
    Input(Input),
    Command(Command),
}

impl TryFrom<CliCommand> for Benchmark {
    type Error = BencherError;

    fn try_from(command: CliCommand) -> Result<Self, Self::Error> {
        Ok(if let Some(cmd) = command.cmd {
            Self::Command(Command::try_from((command.shell, cmd))?)
        } else {
            let input = Input::new()?;
            if input.is_empty() {
                return Err(BencherError::Benchmark);
            }
            Self::Input(input)
        })
    }
}

impl Benchmark {
    pub fn run(&self) -> Result<Output, BencherError> {
        let start = Utc::now();
        let result = match self {
            Self::Input(input) => input.to_string(),
            Self::Command(command) => command.try_into()?,
        };
        Ok(Output { start, result })
    }
}
