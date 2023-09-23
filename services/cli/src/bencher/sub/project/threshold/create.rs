use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonNewThreshold;
use bencher_json::{JsonThreshold, ResourceId};

use super::statistic::Statistic;
use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::threshold::CliThresholdCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub metric_kind: ResourceId,
    pub branch: ResourceId,
    pub testbed: ResourceId,
    pub statistic: Statistic,
    pub backend: Backend,
}

impl TryFrom<CliThresholdCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliThresholdCreate) -> Result<Self, Self::Error> {
        let CliThresholdCreate {
            project,
            metric_kind,
            branch,
            testbed,
            statistic,
            backend,
        } = create;
        Ok(Self {
            project,
            metric_kind,
            branch,
            testbed,
            statistic: statistic.try_into()?,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewThreshold {
    fn from(create: Create) -> Self {
        let Create {
            metric_kind,
            branch,
            testbed,
            statistic,
            ..
        } = create;
        let Statistic {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = statistic;
        Self {
            metric_kind: metric_kind.into(),
            branch: branch.into(),
            testbed: testbed.into(),
            test,
            min_sample_size: min_sample_size.map(Into::into),
            max_sample_size: max_sample_size.map(Into::into),
            window,
            lower_boundary: lower_boundary.map(Into::into),
            upper_boundary: upper_boundary.map(Into::into),
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonThreshold = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_threshold_post()
                        .project(self.project.clone())
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
