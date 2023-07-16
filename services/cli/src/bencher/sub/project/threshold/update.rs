use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonUpdateThreshold;
use bencher_json::{JsonThreshold, ResourceId};
use uuid::Uuid;

use super::statistic::Statistic;
use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::threshold::CliThresholdUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub threshold: Uuid,
    pub statistic: Statistic,
    pub backend: Backend,
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
        Self {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary: lower_boundary.map(Into::into),
            upper_boundary: upper_boundary.map(Into::into),
        }
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonThreshold = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_threshold_patch()
                        .project(self.project.clone())
                        .threshold(self.threshold)
                        .body(self.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
