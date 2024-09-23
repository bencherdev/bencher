use bencher_client::types::{
    Adapter, DateTime, GitHash, JsonAverage, JsonFold, JsonNewReport, JsonReportSettings,
    JsonReportStartPoint, NameId,
};
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::report::{CliReportCreate, CliReportStartPoint},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub branch: NameId,
    pub hash: Option<GitHash>,
    pub start_point: Option<JsonReportStartPoint>,
    pub testbed: NameId,
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
            start_time,
            end_time,
            results,
            adapter,
            average,
            fold,
            backend,
        } = create;
        let CliReportStartPoint {
            start_point_branch,
            start_point_hash,
            start_point_max_versions,
            start_point_reset,
        } = start_point;
        let start_point =
            (start_point_branch.is_some() || start_point_reset).then(|| JsonReportStartPoint {
                branch: start_point_branch.map(Into::into),
                hash: start_point_hash.map(Into::into),
                max_versions: Some(start_point_max_versions),
                reset: Some(start_point_reset),
            });
        Ok(Self {
            project,
            branch: branch.into(),
            hash: hash.map(Into::into),
            start_point: start_point.map(Into::into),
            testbed: testbed.into(),
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
