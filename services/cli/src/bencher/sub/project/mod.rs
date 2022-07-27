use async_trait::async_trait;

use crate::{
    bencher::{
        sub::SubCmd,
        wide::Wide,
    },
    cli::CliProject,
    BencherError,
};

mod create;

#[derive(Debug)]
pub enum Project {
    Create(create::Project),
}

impl TryFrom<CliProject> for Project {
    type Error = BencherError;

    fn try_from(project: CliProject) -> Result<Self, Self::Error> {
        Ok(match project {
            CliProject::Create(create) => Self::Create(create.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Project {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Self::Create(create) => create.exec(wide).await,
        }
    }
}
