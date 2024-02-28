use bencher_client::types::{
    Adapter, DateTime, GitHash, JsonAverage, JsonFold, JsonNewReport, JsonReportSettings, NameId,
};
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::report::CliReportCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub branch: NameId,
    pub hash: Option<GitHash>,
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
            testbed,
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
            testbed,
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
