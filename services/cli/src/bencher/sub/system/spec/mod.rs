use crate::{CliError, bencher::sub::SubCmd, parser::system::spec::CliSpec};

mod create;
mod delete;
mod list;
mod update;
mod view;

#[derive(Debug)]
pub enum Spec {
    List(list::List),
    Create(create::Create),
    View(view::View),
    Update(update::Update),
    Delete(delete::Delete),
}

impl TryFrom<CliSpec> for Spec {
    type Error = CliError;

    fn try_from(spec: CliSpec) -> Result<Self, Self::Error> {
        Ok(match spec {
            CliSpec::List(list) => Self::List(list.try_into()?),
            CliSpec::Create(create) => Self::Create(create.try_into()?),
            CliSpec::View(view) => Self::View(view.try_into()?),
            CliSpec::Update(update) => Self::Update(update.try_into()?),
            CliSpec::Delete(delete) => Self::Delete(delete.try_into()?),
        })
    }
}

impl SubCmd for Spec {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::List(list) => list.exec().await,
            Self::Create(create) => create.exec().await,
            Self::View(view) => view.exec().await,
            Self::Update(update) => update.exec().await,
            Self::Delete(delete) => delete.exec().await,
        }
    }
}
