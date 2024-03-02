use bencher_json::{ModelUuid, ResourceId, ThresholdUuid};

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::threshold::CliThresholdView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub threshold: ThresholdUuid,
    pub model: Option<ModelUuid>,
    pub backend: PubBackend,
}

impl TryFrom<CliThresholdView> for View {
    type Error = CliError;

    fn try_from(view: CliThresholdView) -> Result<Self, Self::Error> {
        let CliThresholdView {
            project,
            threshold,
            model,
            backend,
        } = view;
        Ok(Self {
            project,
            threshold,
            model,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                let mut client = client
                    .proj_threshold_get()
                    .project(self.project.clone())
                    .threshold(self.threshold);

                if let Some(model) = self.model {
                    client = client.model(model);
                }

                client.send().await
            })
            .await?;
        Ok(())
    }
}
