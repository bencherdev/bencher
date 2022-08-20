use std::convert::{
    TryFrom,
    TryInto,
};

use chrono::Utc;

use crate::{
    cli::run::CliRunCommand,
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
pub enum Perf {
    Input(Input),
    Command(Command),
}

impl TryFrom<CliRunCommand> for Perf {
    type Error = BencherError;

    fn try_from(command: CliRunCommand) -> Result<Self, Self::Error> {
        Ok(if let Some(cmd) = command.cmd {
            Self::Command(Command::try_from((command.shell, cmd))?)
        } else {
            let input = Input::new()?;
            if input.is_empty() {
                return Err(BencherError::NoPerf);
            }
            Self::Input(input)
        })
    }
}

impl Perf {
    pub fn run(&self) -> Result<Output, BencherError> {
        let result = match self {
            Self::Input(input) => input.to_string(),
            Self::Command(command) => command.try_into()?,
        };
        Ok(Output { result })
    }
}
