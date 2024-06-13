use bencher_json::{PlotUuid, ResourceId};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::plot::CliPlotDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ResourceId,
    pub plot: PlotUuid,
    pub backend: AuthBackend,
}

impl TryFrom<CliPlotDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliPlotDelete) -> Result<Self, Self::Error> {
        let CliPlotDelete {
            project,
            plot,
            backend,
        } = delete;
        Ok(Self {
            project,
            plot,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_plot_delete()
                    .project(self.project.clone())
                    .plot(self.plot)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
