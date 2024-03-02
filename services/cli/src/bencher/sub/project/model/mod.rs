use crate::{bencher::sub::SubCmd, parser::project::model::CliModel, CliError};

mod view;

#[derive(Debug)]
pub enum Model {
    View(view::View),
}

impl TryFrom<CliModel> for Model {
    type Error = CliError;

    fn try_from(model: CliModel) -> Result<Self, Self::Error> {
        Ok(match model {
            CliModel::View(view) => Self::View(view.try_into()?),
        })
    }
}

impl SubCmd for Model {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::View(view) => view.exec().await,
        }
    }
}
