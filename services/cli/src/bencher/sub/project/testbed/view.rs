use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::testbed::CliTestbedView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub testbed: ResourceId,
    pub backend: PubBackend,
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
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_testbed_get()
                    .project(self.project.clone())
                    .testbed(self.testbed.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
