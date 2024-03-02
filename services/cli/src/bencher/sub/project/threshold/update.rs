use bencher_client::types::JsonUpdateThreshold;
use bencher_json::{ResourceId, ThresholdUuid};

use super::model::Model;
use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::threshold::CliThresholdUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub threshold: ThresholdUuid,
    pub model: Model,
    pub backend: AuthBackend,
}

impl TryFrom<CliThresholdUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliThresholdUpdate) -> Result<Self, Self::Error> {
        let CliThresholdUpdate {
            project,
            threshold,
            model,
            backend,
        } = update;
        Ok(Self {
            project,
            threshold,
            model: model.try_into()?,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateThreshold {
    fn from(update: Update) -> Self {
        let Update { model, .. } = update;
        let Model {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = model;
        #[allow(clippy::inconsistent_struct_constructor)]
        Self {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_threshold_put()
                    .project(self.project.clone())
                    .threshold(self.threshold)
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
