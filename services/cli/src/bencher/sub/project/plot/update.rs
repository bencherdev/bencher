use bencher_client::types::{JsonPlotPatch, JsonPlotPatchNull, JsonUpdatePlot, ParameterFilter};
use bencher_json::{
    BenchmarkUuid, BranchUuid, Index, MeasureUuid, MetricName, PlotUuid, ProjectResourceId,
    ResourceName, TestbedUuid, Window,
    project::plot::{XAxis, YAxis},
};

use super::{PlotError, metric_filter};
use crate::{
    CliError,
    bencher::{
        backend::AuthBackend,
        sub::{SubCmd, project::parameter_filter},
    },
    parser::{
        ElidedOption,
        project::plot::{CliPlotUpdate, CliXAxis, CliYAxis},
    },
};

#[derive(Debug, Clone)]
#[expect(
    clippy::option_option,
    reason = "None = not specified, Some(None) = explicitly unset"
)]
pub struct Update {
    pub project: ProjectResourceId,
    pub plot: PlotUuid,
    pub index: Option<Index>,
    pub title: Option<Option<ResourceName>>,
    pub lower_value: Option<bool>,
    pub upper_value: Option<bool>,
    pub lower_boundary: Option<bool>,
    pub upper_boundary: Option<bool>,
    pub x_axis: Option<XAxis>,
    pub y_axis: Option<YAxis>,
    pub window: Option<Window>,
    pub branches: Option<Vec<BranchUuid>>,
    pub testbeds: Option<Vec<TestbedUuid>>,
    pub benchmarks: Option<Vec<BenchmarkUuid>>,
    pub parameters: Option<ParameterFilter>,
    pub measures: Option<Vec<MeasureUuid>>,
    pub metrics: Option<Vec<MetricName>>,
    pub backend: AuthBackend,
}

impl TryFrom<CliPlotUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliPlotUpdate) -> Result<Self, Self::Error> {
        let CliPlotUpdate {
            project,
            plot,
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
        } = update;
        let parameters = parameters
            .map(parameter_filter)
            .transpose()
            .map_err(PlotError::BadParameters)?;
        let metrics = metric.map(metrics_or_every).transpose()?;
        Ok(Self {
            project,
            plot,
            index,
            title: title.map(Into::into),
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis: x_axis.map(|x_axis| match x_axis {
                CliXAxis::DateTime => XAxis::DateTime,
                CliXAxis::Version => XAxis::Version,
            }),
            y_axis: y_axis.map(|y_axis| match y_axis {
                CliYAxis::Auto => YAxis::Auto,
                CliYAxis::Linear => YAxis::Linear,
                CliYAxis::Log => YAxis::Log,
            }),
            window,
            branches,
            testbeds,
            benchmarks,
            parameters,
            measures,
            metrics,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdatePlot {
    fn from(update: Update) -> Self {
        let Update {
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
        } = update;
        let index = index.map(Into::into);
        let x_axis = x_axis.map(|x_axis| match x_axis {
            XAxis::DateTime => bencher_client::types::XAxis::DateTime,
            XAxis::Version => bencher_client::types::XAxis::Version,
        });
        let y_axis = y_axis.map(|y_axis| match y_axis {
            YAxis::Auto => bencher_client::types::YAxis::Auto,
            YAxis::Linear => bencher_client::types::YAxis::Linear,
            YAxis::Log => bencher_client::types::YAxis::Log,
        });
        let window = window.map(Into::into);
        let branches = branches.map(|branches| branches.into_iter().map(Into::into).collect());
        let testbeds = testbeds.map(|testbeds| testbeds.into_iter().map(Into::into).collect());
        let benchmarks =
            benchmarks.map(|benchmarks| benchmarks.into_iter().map(Into::into).collect());
        let measures = measures.map(|measures| measures.into_iter().map(Into::into).collect());
        let metrics = metrics.map(metric_filter);
        let patch = JsonPlotPatch {
            index,
            title: None,
            lower_value,
            upper_value,
            lower_boundary,
            upper_boundary,
            x_axis,
            y_axis,
            layout: None,
            window,
            branches,
            testbeds,
            benchmarks,
            parameters,
            measures,
            metrics,
            hidden: None,
            focus: None,
        };
        match title {
            Some(Some(title)) => Self {
                subtype_0: Some(JsonPlotPatch {
                    title: Some(title.into()),
                    ..patch
                }),
                subtype_1: None,
            },
            Some(None) => Self {
                subtype_0: None,
                subtype_1: Some(clear_title(patch)),
            },
            None => Self {
                subtype_0: Some(patch),
                subtype_1: None,
            },
        }
    }
}

/// A lone underscore is the empty set, which draws every metric.
fn metrics_or_every(metrics: Vec<ElidedOption<MetricName>>) -> Result<Vec<MetricName>, PlotError> {
    let given = metrics.len();
    let names = metrics
        .into_iter()
        .filter_map(Option::from)
        .collect::<Vec<MetricName>>();
    if names.is_empty() || names.len() == given {
        Ok(names)
    } else {
        Err(PlotError::MetricBesideUnderscore)
    }
}

fn clear_title(patch: JsonPlotPatch) -> JsonPlotPatchNull {
    let JsonPlotPatch {
        index,
        title: _,
        lower_value,
        upper_value,
        lower_boundary,
        upper_boundary,
        x_axis,
        y_axis,
        layout,
        window,
        branches,
        testbeds,
        benchmarks,
        parameters,
        measures,
        metrics,
        hidden,
        focus,
    } = patch;
    JsonPlotPatchNull {
        index,
        title: (),
        lower_value,
        upper_value,
        lower_boundary,
        upper_boundary,
        x_axis,
        y_axis,
        layout,
        window,
        branches,
        testbeds,
        benchmarks,
        parameters,
        measures,
        metrics,
        hidden,
        focus,
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_plot_patch()
                    .project(self.project.clone())
                    .plot(self.plot)
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
    use bencher_client::types::JsonUpdatePlot;
    use clap::Parser as _;

    use super::{PlotError, Update};
    use crate::{CliError, parser::project::plot::CliPlotUpdate};

    const PLOT: [&str; 4] = [
        "update",
        "my-project",
        "4b6eae4c-b05f-4ecf-9d7b-5f60718293a4",
        "--key=bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
    ];

    /// Kills: a repeated `--metric` or `--parameters` the update drops, on the patch with a new
    /// title and on the patch that removes the title, which carry the fields separately.
    #[test]
    fn plot_update_replaces_the_metrics_and_parameters() {
        for title in ["new title", "_"] {
            let body = body(&[
                "--title",
                title,
                "--metric",
                "p99",
                "--metric",
                "mean",
                "--parameters",
                r#"{"size": 64}"#,
                "--parameters",
                r#"{"size": 128}"#,
            ]);
            assert_eq!(
                body["metrics"],
                serde_json::json!(["p99", "mean"]),
                "{body}"
            );
            assert_eq!(
                body["parameters"],
                serde_json::json!([{ "size": 128.0 }, { "size": 64.0 }]),
                "{body}"
            );
        }
    }

    /// Kills: an update with neither flag that sends `null` or an empty list, which the API reads
    /// as clearing the plot's metrics or parameters instead of keeping them.
    #[test]
    fn plot_update_without_the_flags_keeps_them() {
        for title in [None, Some("new title"), Some("_")] {
            let flags = title.map(|title| ["--title", title]);
            let body = body(flags.as_ref().map_or(&[], |flags| flags.as_slice()));
            assert_eq!(body.get("metrics"), None, "{body}");
            assert_eq!(body.get("parameters"), None, "{body}");
        }
    }

    /// Kills: an empty parameters filter the update drops as if no flag were given, which keeps
    /// the plot's filter when the user asked to include every variant again.
    #[test]
    fn plot_update_with_empty_parameters_clears_them() {
        let body = body(&["--parameters", "{}"]);
        assert_eq!(body["parameters"], serde_json::json!([]), "{body}");
    }

    /// Kills: an underscore read as no flag given, which keeps the plot's metrics, or sent as a
    /// metric named `_`, on each of the patches that carry the field.
    #[test]
    fn plot_update_with_an_underscore_clears_the_metrics() {
        for title in [None, Some("new title"), Some("_")] {
            let mut flags = vec!["--metric", "_"];
            flags.extend(title.map(|title| ["--title", title]).into_iter().flatten());
            let body = body(&flags);
            assert_eq!(body["metrics"], serde_json::json!([]), "{body}");
        }
    }

    /// Kills: an underscore beside metric names that is dropped or wins, so the update sends a
    /// set the user did not write instead of refusing it.
    #[test]
    fn plot_update_refuses_an_underscore_beside_metric_names() {
        for flags in [["_", "p99"], ["p99", "_"]] {
            let cli = CliPlotUpdate::try_parse_from(
                PLOT.iter()
                    .copied()
                    .chain(flags.into_iter().flat_map(|metric| ["--metric", metric])),
            )
            .unwrap();
            assert!(
                matches!(
                    Update::try_from(cli),
                    Err(CliError::Plot(PlotError::MetricBesideUnderscore))
                ),
                "{flags:?}"
            );
        }
    }

    fn body(flags: &[&str]) -> serde_json::Value {
        let cli = CliPlotUpdate::try_parse_from(PLOT.iter().chain(flags)).unwrap();
        serde_json::to_value(JsonUpdatePlot::from(Update::try_from(cli).unwrap())).unwrap()
    }
}
