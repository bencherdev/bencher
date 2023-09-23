use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{project::alert::JsonAlertStats, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::alert::CliAlertStats,
    CliError,
};

#[derive(Debug)]
pub struct Stats {
    pub project: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliAlertStats> for Stats {
    type Error = CliError;

    fn try_from(stats: CliAlertStats) -> Result<Self, Self::Error> {
        let CliAlertStats { project, backend } = stats;
        Ok(Self {
            project,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Stats {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonAlertStats = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_alert_stats_get()
                        .project(self.project.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
