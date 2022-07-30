use std::convert::TryFrom;

use async_trait::async_trait;

use super::PROJECTS_PATH;
use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::CliProjectList,
    BencherError,
};

#[derive(Debug)]
pub struct List {
    pub backend: Backend,
}

impl TryFrom<CliProjectList> for List {
    type Error = BencherError;

    fn try_from(list: CliProjectList) -> Result<Self, Self::Error> {
        let CliProjectList { backend } = list;
        Ok(Self {
            backend: Backend::try_from(backend)?,
        })
    }
}

#[async_trait]
impl SubCmd for List {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        self.backend.get(PROJECTS_PATH).await?;
        Ok(())
    }
}
