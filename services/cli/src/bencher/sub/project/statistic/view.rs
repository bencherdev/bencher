use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{project::threshold::JsonStatistic, ResourceId};
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::statistic::CliStatisticView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub statistic: Uuid,
    pub backend: Backend,
}

impl TryFrom<CliStatisticView> for View {
    type Error = CliError;

    fn try_from(view: CliStatisticView) -> Result<Self, Self::Error> {
        let CliStatisticView {
            project,
            statistic,
            backend,
        } = view;
        Ok(Self {
            project,
            statistic,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonStatistic = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_statistic_get()
                        .project(self.project.clone())
                        .statistic(self.statistic)
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
