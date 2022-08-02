use std::{
    convert::TryFrom,
    path::Path,
};

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::CliTestbedView,
    BencherError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub testbed: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliTestbedView> for View {
    type Error = BencherError;

    fn try_from(view: CliTestbedView) -> Result<Self, Self::Error> {
        let CliTestbedView {
            project,
            testbed,
            backend,
        } = view;
        Ok(Self {
            project,
            testbed,
            backend: Backend::try_from(backend)?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        self.backend
            .get(&format!(
                "/v0/projects/{}/testbeds/{}",
                self.project.as_str(),
                self.testbed.as_str()
            ))
            .await?;
        Ok(())
    }
}
