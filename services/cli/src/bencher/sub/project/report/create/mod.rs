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
mod bmf_version;
mod fold;
mod measures;
mod thresholds;

pub use bmf_version::{BmfVersionError, DeclaredBmfVersion, project_bmf_version};
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

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let json_new_report = self.new_report(self.bmf_version().await?)?;
        let _json = self
            .backend
            .send(|client| {
                let json_new_report = json_new_report.clone();
                async move {
                    client
                        .proj_report_post()
                        .project(self.project.clone())
                        .body(json_new_report)
                        .send()
                        .await
                }
            })
            .await?;
        Ok(())
    }
}

impl Create {
    /// The version an explicit JSON adapter declares, else the project's, read without printing
    /// it, so the report is all the command prints.
    async fn bmf_version(&self) -> Result<DeclaredBmfVersion, BmfVersionError> {
        if let Some(declared) = DeclaredBmfVersion::from_adapter(self.adapter) {
            return Ok(declared);
        }
        project_bmf_version(&self.backend.clone().log(false), &self.project)
            .await
            .map(DeclaredBmfVersion::Project)
    }

    fn new_report(&self, declared: DeclaredBmfVersion) -> Result<JsonNewReport, ThresholdsError> {
        let Self {
            project: _,
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
            measures,
            backend: _,
        } = self;
        Ok(JsonNewReport {
            branch: branch.clone().into(),
            hash: hash.clone(),
            start_point: start_point.clone(),
            testbed: testbed.clone().into(),
            thresholds: thresholds.at(declared)?,
            start_time: start_time.clone(),
            end_time: end_time.clone(),
            results: results.clone(),
            bmf_version: Some(declared.version().into()),
            settings: Some(JsonReportSettings {
                adapter: *adapter,
                average: *average,
                fold: *fold,
                measures: measures.clone(),
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::BmfVersion;
    use clap::Parser as _;

    use super::{Create, DeclaredBmfVersion, ThresholdsError};
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
        let json_new_report = Create::try_from(cli)
            .unwrap()
            .new_report(DeclaredBmfVersion::Project(BmfVersion::V0))
            .unwrap();
        assert_eq!(
            serde_json::to_value(json_new_report).unwrap()["settings"]["measures"],
            serde_json::json!(measures)
        );
    }

    const PLAIN: [&str; 6] = [
        "--threshold-measure",
        "latency",
        "--threshold-test",
        "t_test",
        "--threshold-upper-boundary",
        "0.99",
    ];
    const METRIC: [&str; 2] = ["--threshold-metric", "p99"];

    /// Kills: thresholds the request drops, built without the version, or a report that declares
    /// no version or another one.
    #[test]
    fn report_create_sends_its_thresholds_at_the_version_it_declares() {
        let model = serde_json::json!({ "test": "t_test", "upper_boundary": 0.99 });
        for (flags, bmf_version, thresholds) in [
            (
                &PLAIN[..],
                BmfVersion::V0,
                serde_json::json!({ "latency": model }),
            ),
            (
                &PLAIN[..],
                BmfVersion::V1,
                serde_json::json!({ "latency": model }),
            ),
            (
                &[&PLAIN[..], &METRIC].concat(),
                BmfVersion::V1,
                serde_json::json!([{ "measure": "latency", "metric": "p99", "model": model }]),
            ),
        ] {
            let body = serde_json::to_value(
                create(flags)
                    .new_report(DeclaredBmfVersion::Project(bmf_version))
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(
                (&body["thresholds"]["models"], &body["bmf_version"]),
                (&thresholds, &serde_json::json!(u8::from(bmf_version))),
                "{flags:?} {bmf_version}"
            );
        }
    }

    /// Kills: a threshold metric that a report read at BMF version 0 sends.
    #[test]
    fn report_create_refuses_a_threshold_metric_at_version_0() {
        let result = create(&[&PLAIN[..], &METRIC].concat())
            .new_report(DeclaredBmfVersion::Project(BmfVersion::V0));
        assert!(
            matches!(result, Err(ThresholdsError::MetricAtVersion0(_))),
            "{result:?}"
        );
    }

    fn create(flags: &[&str]) -> Create {
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
        .iter()
        .chain(flags)
        .copied();
        Create::try_from(CliReportCreate::try_parse_from(args).unwrap()).unwrap()
    }
}
