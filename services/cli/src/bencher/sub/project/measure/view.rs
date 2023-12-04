use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonMeasure, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::measure::CliMeasureView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub measure: ResourceId,
    pub backend: Backend,
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
        let _json: JsonMeasure = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_measure_get()
                        .project(self.project.clone())
                        .measure(self.measure.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
