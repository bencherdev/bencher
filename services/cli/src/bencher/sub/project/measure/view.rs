use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::measure::CliMeasureView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub measure: ResourceId,
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

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .as_ref()
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
