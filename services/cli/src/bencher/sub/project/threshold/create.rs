use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonNewThreshold;
use bencher_json::{NameId, ResourceId};

use super::statistic::Statistic;
use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::threshold::CliThresholdCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub branch: NameId,
    pub testbed: NameId,
    pub measure: NameId,
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
            measure,
            statistic,
            backend,
        } = create;
        Ok(Self {
            project,
            branch,
            testbed,
            measure,
            statistic: statistic.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewThreshold {
    fn from(create: Create) -> Self {
        let Create {
            branch,
            testbed,
            statistic,
            measure,
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
            branch: branch.into(),
            testbed: testbed.into(),
            measure: measure.into(),
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_threshold_post()
                    .project(self.project.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
