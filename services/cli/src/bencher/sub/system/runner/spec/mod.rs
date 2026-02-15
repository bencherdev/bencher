use crate::{CliError, bencher::sub::SubCmd, parser::system::runner::CliRunnerSpec};

mod add;
mod list;
mod remove;

#[derive(Debug)]
pub enum Spec {
    List(list::List),
    Add(add::Add),
    Remove(remove::Remove),
}

impl TryFrom<CliRunnerSpec> for Spec {
    type Error = CliError;

    fn try_from(spec: CliRunnerSpec) -> Result<Self, Self::Error> {
        Ok(match spec {
            CliRunnerSpec::List(list) => Self::List(list.try_into()?),
            CliRunnerSpec::Add(add) => Self::Add(add.try_into()?),
            CliRunnerSpec::Remove(remove) => Self::Remove(remove.try_into()?),
        })
    }
}

impl SubCmd for Spec {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Add(add) => add.exec().await,
            Self::Remove(remove) => remove.exec().await,
        }
    }
}
