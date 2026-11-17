use bencher_client::types::{JsonNewPlot, ParameterFilter};
use bencher_json::{
    BenchmarkUuid, BranchUuid, Index, MeasureUuid, MetricName, ProjectResourceId, ResourceName,
    TestbedUuid, Window,
    project::plot::{XAxis, YAxis},
};

use super::{PlotError, metric_filter};
use crate::{
    CliError,
    bencher::{
        backend::AuthBackend,
        sub::{SubCmd, project::parameter_filter},
    },
    parser::project::plot::{CliPlotCreate, CliXAxis, CliYAxis},
};

#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool is an independent user flag"
)]
pub struct Create {
    pub project: ProjectResourceId,
    pub index: Option<Index>,
    pub title: Option<ResourceName>,
    pub lower_value: bool,
    pub upper_value: bool,
    pub lower_boundary: bool,
    pub upper_boundary: bool,
    pub x_axis: XAxis,
    pub y_axis: YAxis,
    pub window: Window,
    pub branches: Vec<BranchUuid>,
    pub testbeds: Vec<TestbedUuid>,
    pub benchmarks: Vec<BenchmarkUuid>,
    pub parameters: Option<ParameterFilter>,
    pub measures: Vec<MeasureUuid>,
    pub metrics: Vec<MetricName>,
    pub backend: AuthBackend,
}

impl TryFrom<CliPlotCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliPlotCreate) -> Result<Self, Self::Error> {
        let CliPlotCreate {
            project,
            index,
            title,
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis,
            y_axis,
            window,
            branches,
            testbeds,
            benchmarks,
            parameters,
            measures,
            metric,
            backend,
        } = create;
        let parameters = (!parameters.is_empty())
            .then(|| parameter_filter(parameters))
            .transpose()
            .map_err(PlotError::BadParameters)?;
        Ok(Self {
            project,
            index,
            title,
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis: match x_axis {
                CliXAxis::DateTime => XAxis::DateTime,
                CliXAxis::Version => XAxis::Version,
            },
            y_axis: match y_axis {
                CliYAxis::Auto => YAxis::Auto,
                CliYAxis::Linear => YAxis::Linear,
                CliYAxis::Log => YAxis::Log,
            },
            window,
            branches,
            testbeds,
            benchmarks,
            parameters,
            measures,
            metrics: metric,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewPlot {
    fn from(create: Create) -> Self {
        let Create {
            index,
            title,
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis,
            y_axis,
            window,
            branches,
            testbeds,
            benchmarks,
            parameters,
            measures,
            metrics,
            ..
        } = create;
        Self {
            index: index.map(Into::into),
            title: title.map(Into::into),
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis: match x_axis {
                XAxis::DateTime => bencher_client::types::XAxis::DateTime,
                XAxis::Version => bencher_client::types::XAxis::Version,
            },
            y_axis: match y_axis {
                YAxis::Auto => bencher_client::types::YAxis::Auto,
                YAxis::Linear => bencher_client::types::YAxis::Linear,
                YAxis::Log => bencher_client::types::YAxis::Log,
            },
            layout: None,
            window: window.into(),
            branches: branches.into_iter().map(Into::into).collect(),
            testbeds: testbeds.into_iter().map(Into::into).collect(),
            benchmarks: benchmarks.into_iter().map(Into::into).collect(),
            parameters,
            measures: measures.into_iter().map(Into::into).collect(),
            metrics: (!metrics.is_empty()).then(|| metric_filter(metrics)),
            hidden: None,
            focus: None,
        }
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_plot_post()
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
    use bencher_client::types::JsonNewPlot;
    use clap::Parser as _;

    use super::Create;
    use crate::parser::project::plot::CliPlotCreate;

    const PLOT: [&str; 15] = [
        "create",
        "my-project",
        "--x-axis",
        "date_time",
        "--window",
        "2592000",
        "--branches",
        "0d2a6a0e-7c1b-4a8e-9f3d-1b2c3d4e5f60",
        "--testbeds",
        "1e3b7b1f-8d2c-4b9f-8a4e-2c3d4e5f6071",
        "--benchmarks",
        "2f4c8c2a-9e3d-4cad-9b5f-3d4e5f607182",
        "--measures",
        "3a5d9d3b-af4e-4dbe-8c6a-4e5f60718293",
        "--key=bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
    ];

    /// Kills: a repeated `--metric` or `--parameters` the conversion drops or cuts to its first,
    /// and metrics sent in an order other than the one given.
    #[test]
    fn plot_create_sends_every_metric_and_parameter_set() {
        let body = body(&[
            "--metric",
            "p99",
            "--metric",
            "mean",
            "--parameters",
            r#"{"size": 64}"#,
            "--parameters",
            r#"{"size": 128, "mode": "fast"}"#,
        ]);
        assert_eq!(body["metrics"], serde_json::json!(["p99", "mean"]));
        assert_eq!(
            body["parameters"],
            serde_json::json!([{ "mode": "fast", "size": 128.0 }, { "size": 64.0 }])
        );
    }

    fn body(flags: &[&str]) -> serde_json::Value {
        let cli = CliPlotCreate::try_parse_from(PLOT.iter().chain(flags)).unwrap();
        serde_json::to_value(JsonNewPlot::from(Create::try_from(cli).unwrap())).unwrap()
    }
}
