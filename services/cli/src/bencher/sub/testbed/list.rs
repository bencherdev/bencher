use std::convert::TryFrom;

use async_trait::async_trait;

use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::CliTestbedList,
    BencherError,
};

#[derive(Debug)]
pub struct List {
    pub project: String,
    pub backend: Backend,
}

impl TryFrom<CliTestbedList> for List {
    type Error = BencherError;

    fn try_from(list: CliTestbedList) -> Result<Self, Self::Error> {
        let CliTestbedList { project, backend } = list;
        Ok(Self {
            project,
            backend: Backend::try_from(backend)?,
        })
    }
}

#[async_trait]
impl SubCmd for List {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        self.backend
            .get(&format!("/v0/projects/{}/testbeds", self.project))
            .await?;
        Ok(())
    }
}
