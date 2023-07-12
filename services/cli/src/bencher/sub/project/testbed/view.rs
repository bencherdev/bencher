use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonTestbed, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::testbed::CliTestbedView,
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
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonTestbed = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_testbed_get()
                        .project(self.project.clone())
                        .testbed(self.testbed.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
