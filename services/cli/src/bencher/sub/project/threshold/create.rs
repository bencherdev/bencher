use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{project::perf::JsonPerfKind, JsonNewThreshold, ResourceId};
use uuid::Uuid;

use super::statistic::Statistic;
use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::project::threshold::CliThresholdCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub branch: Uuid,
    pub testbed: Uuid,
    pub kind: JsonPerfKind,
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
            kind: kind.into(),
            statistic: statistic.try_into()?,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewThreshold {
    fn from(create: Create) -> Self {
        let Create {
            project: _,
            branch,
            testbed,
            kind,
            statistic,
            backend: _,
        } = create;
        Self {
            branch,
            testbed,
            kind,
            statistic: statistic.into(),
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
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
