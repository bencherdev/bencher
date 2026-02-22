#[cfg(feature = "plus")]
use bencher_json::SpecUuid;
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
    #[cfg(feature = "plus")]
    pub spec: Option<SpecUuid>,
    pub backend: PubBackend,
}

impl TryFrom<CliTestbedView> for View {
    type Error = CliError;

    fn try_from(view: CliTestbedView) -> Result<Self, Self::Error> {
        let CliTestbedView {
            project,
            testbed,
            #[cfg(feature = "plus")]
            spec,
            backend,
        } = view;
        Ok(Self {
            project,
            testbed,
            #[cfg(feature = "plus")]
            spec,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                #[cfg_attr(not(feature = "plus"), expect(unused_mut))]
                let mut client = client
                    .proj_testbed_get()
                    .project(self.project.clone())
                    .testbed(self.testbed.clone());

                #[cfg(feature = "plus")]
                if let Some(spec) = self.spec {
                    client = client.spec(spec);
                }

                client.send().await
            })
            .await?;
        Ok(())
    }
}
