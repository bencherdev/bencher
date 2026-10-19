use bencher_client::types::{
    Adapter, DateTime, GitHash, JsonAverage, JsonFold, JsonNewReport, JsonReportSettings,
    JsonUpdateStartPoint, MeasureFilter,
};
use bencher_json::{BranchNameId, ProjectResourceId, TestbedNameId};

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
mod measures;
mod thresholds;

pub use measures::measure_filter;
pub use thresholds::{Thresholds, ThresholdsError};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ProjectResourceId,
    pub branch: BranchNameId,
    pub hash: Option<GitHash>,
    pub start_point: Option<JsonUpdateStartPoint>,
    pub testbed: TestbedNameId,
    pub thresholds: Thresholds,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub results: Vec<String>,
    pub adapter: Option<Adapter>,
    pub average: Option<JsonAverage>,
    pub fold: Option<JsonFold>,
    pub measures: Option<MeasureFilter>,
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
            measure,
            backend,
        } = create;
        Ok(Self {
            project,
            branch,
            hash: hash.map(Into::into),
            start_point: StartPoint::from(start_point).into(),
            testbed,
            thresholds: thresholds.try_into().map_err(CliError::Thresholds)?,
            start_time: start_time.into(),
            end_time: end_time.into(),
            results,
            adapter: adapter.map(Into::into),
            average: average.map(Into::into),
            fold: fold.map(Into::into),
            measures: measure_filter(measure),
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
            measures,
            ..
        } = create;
        Self {
            branch: branch.into(),
            hash,
            start_point,
            testbed: testbed.into(),
            thresholds: None,
            start_time,
            end_time,
            results,
            // The CLI declares no BMF version yet, so the report is read at the
            // project's default.
            bmf_version: None,
            settings: Some(JsonReportSettings {
                adapter,
                average,
                fold,
                measures,
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

#[cfg(test)]
mod tests {
    use bencher_client::types::JsonNewReport;
    use clap::Parser as _;

    use super::Create;
    use crate::parser::project::report::CliReportCreate;

    /// Kills: a `--measure` list that the parse discards, a report whose request sends no list,
    /// and a list of 65 that the CLI refuses or cuts instead of sending it as given.
    #[test]
    fn report_create_sends_65_listed_measures_unchanged() {
        let measures = ["latency".to_owned(), "Wall Clock".to_owned()]
            .into_iter()
            .chain((2..65).map(|n| format!("m{n}")))
            .collect::<Vec<_>>();
        let args = [
            "create",
            "my-project",
            "--branch",
            "main",
            "--testbed",
            "localhost",
            "--start-time",
            "2024-01-01T00:00:00Z",
            "--end-time",
            "2024-01-01T00:01:00Z",
            "--key",
            "bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
        ]
        .into_iter()
        .chain(
            measures
                .iter()
                .flat_map(|measure| ["--measure", measure.as_str()]),
        );
        let cli = CliReportCreate::try_parse_from(args).unwrap();
        let json_new_report = JsonNewReport::from(Create::try_from(cli).unwrap());
        assert_eq!(
            serde_json::to_value(json_new_report).unwrap()["settings"]["measures"],
            serde_json::json!(measures)
        );
    }
}
