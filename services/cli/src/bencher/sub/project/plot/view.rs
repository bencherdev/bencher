use bencher_json::{PlotUuid, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::plot::CliPlotView,
};

#[derive(Debug)]
pub struct View {
    pub project: ProjectResourceId,
    pub plot: PlotUuid,
    pub backend: PubBackend,
}

impl TryFrom<CliPlotView> for View {
    type Error = CliError;

    fn try_from(view: CliPlotView) -> Result<Self, Self::Error> {
        let CliPlotView {
            project,
            plot,
            backend,
        } = view;
        Ok(Self {
            project,
            plot,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_plot_get()
                    .project(self.project.clone())
                    .plot(self.plot)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
