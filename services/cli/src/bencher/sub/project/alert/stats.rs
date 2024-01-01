use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::alert::CliAlertStats,
    CliError,
};

#[derive(Debug)]
pub struct Stats {
    pub project: ResourceId,
    pub backend: PubBackend,
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
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_alert_stats_get()
                    .project(self.project.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
