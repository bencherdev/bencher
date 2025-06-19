use bencher_client::types::{
    Adapter, DateTime, GitHash, JsonAverage, JsonFold, JsonNewReport, JsonReportSettings,
    JsonUpdateStartPoint, NameId,
};
use bencher_json::ResourceId;

use crate::{
    CliError,
    bencher::{
        backend::AuthBackend,
        sub::{SubCmd, project::branch::start_point::StartPoint},
    },
    parser::project::report::CliReportCreate,
};

mod adapter;
mod average;
mod fold;
mod thresholds;

pub use thresholds::{Thresholds, ThresholdsError};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub branch: NameId,
    pub hash: Option<GitHash>,
    pub start_point: Option<JsonUpdateStartPoint>,
    pub testbed: NameId,
    pub thresholds: Thresholds,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub results: Vec<String>,
    pub adapter: Option<Adapter>,
    pub average: Option<JsonAverage>,
    pub fold: Option<JsonFold>,
    pub backend: AuthBackend,
}

impl TryFrom<CliReportCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliReportCreate) -> Result<Self, Self::Error> {
        let CliReportCreate {
            project,
            branch,
            hash,
            start_point,
            testbed,
            thresholds,
            start_time,
            end_time,
            results,
            adapter,
            average,
            fold,
            backend,
        } = create;
        Ok(Self {
            project,
            branch: branch.into(),
            hash: hash.map(Into::into),
            start_point: StartPoint::from(start_point).into(),
            testbed: testbed.into(),
            thresholds: thresholds.try_into().map_err(CliError::Thresholds)?,
            start_time: start_time.into(),
            end_time: end_time.into(),
            results,
            adapter: adapter.map(Into::into),
            average: average.map(Into::into),
            fold: fold.map(Into::into),
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewReport {
    fn from(create: Create) -> Self {
        let Create {
            branch,
            hash,
            start_point,
            testbed,
            start_time,
            end_time,
            results,
            adapter,
            average,
            fold,
            ..
        } = create;
        Self {
            branch,
            hash,
            start_point,
            testbed,
            thresholds: None,
            start_time,
            end_time,
            results,
            settings: Some(JsonReportSettings {
                adapter,
                average,
                fold,
            }),
        }
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_report_post()
                    .project(self.project.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
