use std::convert::{TryFrom, TryInto};

use crate::{cli::project::run::CliRunCommand, CliError};

mod command;
mod flag;
mod input;
mod output;
mod shell;

use command::Command;
use input::Input;
pub use output::Output;

use super::BENCHER_CMD;

#[derive(Debug)]
pub enum Runner {
    Input(Input),
    Command(Command),
}

impl TryFrom<CliRunCommand> for Runner {
    type Error = CliError;

    fn try_from(command: CliRunCommand) -> Result<Self, Self::Error> {
        if let Some(cmd) = command.cmd {
            Ok(Self::Command(Command::try_from((command.shell, cmd))?))
        } else {
            let input = Input::new()?;
            if input.is_empty() {
                if let Ok(cmd) = std::env::var(BENCHER_CMD) {
                    Ok(Self::Command(Command::try_from((command.shell, cmd))?))
                } else {
                    Err(CliError::NoCommand)
                }
            } else {
                Ok(Self::Input(input))
            }
        }
    }
}

impl Runner {
    pub fn run(&self) -> Result<Output, CliError> {
        let result = match self {
            Self::Input(input) => input.to_string(),
            Self::Command(command) => command.try_into()?,
        };
        Ok(Output { result })
    }
}
