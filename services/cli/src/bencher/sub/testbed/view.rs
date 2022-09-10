use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::testbed::CliTestbedView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub testbed: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliTestbedView> for View {
    type Error = CliError;

    fn try_from(view: CliTestbedView) -> Result<Self, Self::Error> {
        let CliTestbedView {
            project,
            testbed,
            backend,
        } = view;
        Ok(Self {
            project,
            testbed,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend
            .get(&format!(
                "/v0/projects/{}/testbeds/{}",
                self.project,
                self.testbed.as_str()
            ))
            .await?;
        Ok(())
    }
}
