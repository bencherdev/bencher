use bencher_client::types::{JsonPlotPatch, JsonPlotPatchNull, JsonUpdatePlot};
use bencher_json::{
    BenchmarkUuid, BranchUuid, Index, MeasureUuid, PlotUuid, ProjectResourceId, ResourceName,
    TestbedUuid, Window, project::plot::XAxis,
};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::plot::{CliPlotUpdate, CliXAxis},
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
    pub window: Option<Window>,
    pub branches: Option<Vec<BranchUuid>>,
    pub testbeds: Option<Vec<TestbedUuid>>,
    pub benchmarks: Option<Vec<BenchmarkUuid>>,
    pub measures: Option<Vec<MeasureUuid>>,
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
            window,
            branches,
            testbeds,
            benchmarks,
            measures,
            backend,
        } = update;
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
            window,
            branches,
            testbeds,
            benchmarks,
            measures,
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
            window,
            branches,
            testbeds,
            benchmarks,
            measures,
            ..
        } = update;
        let index = index.map(Into::into);
        let x_axis = x_axis.map(|x_axis| match x_axis {
            XAxis::DateTime => bencher_client::types::XAxis::DateTime,
            XAxis::Version => bencher_client::types::XAxis::Version,
        });
        let window = window.map(Into::into);
        let branches = branches.map(|branches| branches.into_iter().map(Into::into).collect());
        let testbeds = testbeds.map(|testbeds| testbeds.into_iter().map(Into::into).collect());
        let benchmarks =
            benchmarks.map(|benchmarks| benchmarks.into_iter().map(Into::into).collect());
        let measures = measures.map(|measures| measures.into_iter().map(Into::into).collect());
        match title {
            Some(Some(title)) => Self {
                subtype_0: Some(JsonPlotPatch {
                    index,
                    title: Some(title.into()),
                    lower_value,
                    upper_value,
                    lower_boundary,
                    upper_boundary,
                    x_axis,
                    window,
                    branches,
                    testbeds,
                    benchmarks,
                    measures,
                }),
                subtype_1: None,
            },
            Some(None) => Self {
                subtype_0: None,
                subtype_1: Some(JsonPlotPatchNull {
                    index,
                    title: (),
                    lower_value,
                    upper_value,
                    lower_boundary,
                    upper_boundary,
                    x_axis,
                    window,
                    branches,
                    testbeds,
                    benchmarks,
                    measures,
                }),
            },
            None => Self {
                subtype_0: Some(JsonPlotPatch {
                    index,
                    title: None,
                    lower_value,
                    upper_value,
                    lower_boundary,
                    upper_boundary,
                    x_axis,
                    window,
                    branches,
                    testbeds,
                    benchmarks,
                    measures,
                }),
                subtype_1: None,
            },
        }
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
