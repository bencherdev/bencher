use bencher_client::types::JsonNewPlot;
use bencher_json::{
    BenchmarkUuid, BranchUuid, Index, MeasureUuid, ProjectResourceId, ResourceName, TestbedUuid,
    Window, project::plot::XAxis,
};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::plot::{CliPlotCreate, CliXAxis},
};

#[derive(Debug, Clone)]
#[expect(clippy::struct_excessive_bools)]
pub struct Create {
    pub project: ProjectResourceId,
    pub index: Option<Index>,
    pub title: Option<ResourceName>,
    pub lower_value: bool,
    pub upper_value: bool,
    pub lower_boundary: bool,
    pub upper_boundary: bool,
    pub x_axis: XAxis,
    pub window: Window,
    pub branches: Vec<BranchUuid>,
    pub testbeds: Vec<TestbedUuid>,
    pub benchmarks: Vec<BenchmarkUuid>,
    pub measures: Vec<MeasureUuid>,
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
            window,
            branches,
            testbeds,
            benchmarks,
            measures,
            backend,
        } = create;
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
            window,
            branches,
            testbeds,
            benchmarks,
            measures,
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
            window,
            branches,
            testbeds,
            benchmarks,
            measures,
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
            window: window.into(),
            branches: branches.into_iter().map(Into::into).collect(),
            testbeds: testbeds.into_iter().map(Into::into).collect(),
            benchmarks: benchmarks.into_iter().map(Into::into).collect(),
            measures: measures.into_iter().map(Into::into).collect(),
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
