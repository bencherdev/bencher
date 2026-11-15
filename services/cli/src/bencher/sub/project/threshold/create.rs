use bencher_client::types::{JsonNewThreshold, ParameterFilter};
use bencher_json::{BranchNameId, MeasureNameId, MetricName, ProjectResourceId, TestbedNameId};

use super::{ThresholdError, model::Model};
use crate::{
    CliError,
    bencher::{
        backend::AuthBackend,
        sub::{SubCmd, project::parameter_filter},
    },
    parser::project::threshold::CliThresholdCreate,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ProjectResourceId,
    pub branch: BranchNameId,
    pub testbed: TestbedNameId,
    pub measure: MeasureNameId,
    pub metric: Option<MetricName>,
    pub parameters: Option<ParameterFilter>,
    pub model: Model,
    pub backend: AuthBackend,
}

impl TryFrom<CliThresholdCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliThresholdCreate) -> Result<Self, Self::Error> {
        let CliThresholdCreate {
            project,
            branch,
            testbed,
            measure,
            metric,
            parameters,
            model,
            backend,
        } = create;
        let parameters = parameters
            .map(|set| parameter_filter(vec![set]))
            .transpose()
            .map_err(ThresholdError::BadParameters)?;
        Ok(Self {
            project,
            branch,
            testbed,
            measure,
            metric,
            parameters,
            model: model.try_into()?,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewThreshold {
    fn from(create: Create) -> Self {
        let Create {
            branch,
            testbed,
            measure,
            metric,
            parameters,
            model,
            ..
        } = create;
        let Model {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = model;
        Self {
            branch: branch.into(),
            testbed: testbed.into(),
            parameters,
            measure: measure.into(),
            metric: metric.map(Into::into),
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use bencher_client::types::JsonNewThreshold;
    use clap::Parser as _;

    use super::Create;
    use crate::parser::project::threshold::CliThresholdCreate;

    const THRESHOLD: [&str; 13] = [
        "create",
        "my-project",
        "--branch",
        "main",
        "--testbed",
        "localhost",
        "--measure",
        "latency",
        "--test",
        "t_test",
        "--upper-boundary",
        "0.99",
        "--key=bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
    ];

    /// Kills: a `--metric` or `--parameters` the conversion drops, which sends the bare `value`
    /// threshold of every variant, and parameters sent as a bare set instead of a filter of one.
    #[test]
    fn threshold_create_sends_its_metric_and_parameters() {
        let body = body(&[
            "--metric",
            "p99",
            "--parameters",
            r#"{"size": 64, "mode": "fast"}"#,
        ]);
        assert_eq!(body["metric"], serde_json::json!("p99"));
        assert_eq!(
            body["parameters"],
            serde_json::json!([{ "mode": "fast", "size": 64.0 }])
        );
    }

    fn body(flags: &[&str]) -> serde_json::Value {
        let cli = CliThresholdCreate::try_parse_from(THRESHOLD.iter().chain(flags)).unwrap();
        serde_json::to_value(JsonNewThreshold::from(Create::try_from(cli).unwrap())).unwrap()
    }
}
