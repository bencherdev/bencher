use async_trait::async_trait;

use crate::{
    bencher::{
        sub::SubCmd,
        wide::Wide,
    },
    cli::report::CliReport,
    BencherError,
};

// mod create;
mod list;
// mod view;

#[derive(Debug)]
pub enum Report {
    List(list::List),
    // Create(create::Create),
    // View(view::View),
}

impl TryFrom<CliReport> for Report {
    type Error = BencherError;

    fn try_from(report: CliReport) -> Result<Self, Self::Error> {
        Ok(match report {
            CliReport::List(list) => Self::List(list.try_into()?),
            // CliReport::Create(create) => Self::Create(create.try_into()?),
            // CliReport::View(view) => Self::View(view.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Report {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Self::List(list) => list.exec(wide).await,
            // Self::Create(create) => create.exec(wide).await,
            // Self::View(create) => create.exec(wide).await,
        }
    }
}
