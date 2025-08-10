use bencher_json::{ProjectResourceId, TestbedResourceId};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::testbed::CliTestbedView,
};

#[derive(Debug)]
pub struct View {
    pub project: ProjectResourceId,
    pub testbed: TestbedResourceId,
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
