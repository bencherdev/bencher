use std::convert::TryFrom;

use bencher_client::types::JsonUpdateThreshold;
use bencher_json::{ResourceId, ThresholdUuid};

use super::statistic::Statistic;
use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::threshold::CliThresholdUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub threshold: ThresholdUuid,
    pub statistic: Statistic,
    pub backend: AuthBackend,
}

impl TryFrom<CliThresholdUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliThresholdUpdate) -> Result<Self, Self::Error> {
        let CliThresholdUpdate {
            project,
            threshold,
            statistic,
            backend,
        } = update;
        Ok(Self {
            project,
            threshold,
            statistic: statistic.try_into()?,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateThreshold {
    fn from(update: Update) -> Self {
        let Update { statistic, .. } = update;
        let Statistic {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = statistic;
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
