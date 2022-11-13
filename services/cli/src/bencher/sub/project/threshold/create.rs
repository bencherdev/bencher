use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonNewThreshold, ResourceId};
use uuid::Uuid;

use super::statistic::Statistic;
use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::threshold::CliThresholdCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub branch: Uuid,
    pub testbed: Uuid,
    pub metric_kind: ResourceId,
    pub statistic: Statistic,
    pub backend: Backend,
}

impl TryFrom<CliThresholdCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliThresholdCreate) -> Result<Self, Self::Error> {
        let CliThresholdCreate {
            project,
            branch,
            testbed,
            kind,
            statistic,
            backend,
        } = create;
        Ok(Self {
            project,
            branch,
            testbed,
            metric_kind: kind,
            statistic: statistic.try_into()?,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewThreshold {
    fn from(create: Create) -> Self {
        let Create {
            branch,
            testbed,
            metric_kind,
            statistic,
            ..
        } = create;
        Self {
            branch,
            testbed,
            metric_kind,
            statistic: statistic.into(),
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let threshold: JsonNewThreshold = self.clone().into();
        self.backend
            .post(
                &format!("/v0/projects/{}/thresholds", self.project),
                &threshold,
            )
            .await?;
        Ok(())
    }
}
