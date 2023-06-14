use std::{convert::TryFrom, path::PathBuf};

use async_trait::async_trait;
use bencher_json::{ResourceId};
use serde_json::json;
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::report::CliReportUpload,
    CliError,
};

#[derive(Debug)]
pub struct Upload{
    pub project: ResourceId,
    pub report: Uuid,
    pub backend: Backend,
    pub perf_data_path: PathBuf
}

impl TryFrom<CliReportUpload> for Upload {
    type Error = CliError;

    fn try_from(upload: CliReportUpload) -> Result<Self, Self::Error> {
        let CliReportUpload{
            project,
            report,
            backend,
            perf_data_path
        } = upload;
        Ok(Self {
            project,
            report,
            backend: backend.try_into()?,
            perf_data_path
        })
    }
}

#[async_trait]
impl SubCmd for Upload {
    async fn exec(&self) -> Result<(), CliError> {
        let perf_data_json = json!(self.perf_data_path);
        self.backend
            .post(&format!(
                "/v0/projects/{}/reports/{}",
                self.project, self.report
            ), &perf_data_json)
            .await?;
        Ok(())
    }
}
