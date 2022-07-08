use std::convert::TryFrom;
use std::convert::TryInto;

use chrono::Utc;

use crate::cli::clap::CliCommand;
use crate::BencherError;

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
        Ok(match self {
            Self::Input(input) => Output {
                start: None,
                result: input.to_string(),
            },
            Self::Command(command) => Output {
                start: Some(Utc::now()),
                result: command.try_into()?,
            },
        })
    }
}
