use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{ResourceId, ThresholdUuid};

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::threshold::CliThresholdView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub threshold: ThresholdUuid,
    pub backend: PubBackend,
}

impl TryFrom<CliThresholdView> for View {
    type Error = CliError;

    fn try_from(view: CliThresholdView) -> Result<Self, Self::Error> {
        let CliThresholdView {
            project,
            threshold,
            backend,
        } = view;
        Ok(Self {
            project,
            threshold,
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
                    .proj_threshold_get()
                    .project(self.project.clone())
                    .threshold(self.threshold)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
