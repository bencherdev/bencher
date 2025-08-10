use bencher_json::{MeasureResourceId, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::measure::CliMeasureView,
};

#[derive(Debug)]
pub struct View {
    pub project: ProjectResourceId,
    pub measure: MeasureResourceId,
    pub backend: PubBackend,
}

impl TryFrom<CliMeasureView> for View {
    type Error = CliError;

    fn try_from(view: CliMeasureView) -> Result<Self, Self::Error> {
        let CliMeasureView {
            project,
            measure,
            backend,
        } = view;
        Ok(Self {
            project,
            measure,
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
                    .proj_measure_get()
                    .project(self.project.clone())
                    .measure(self.measure.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
