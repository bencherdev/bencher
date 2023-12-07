use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::measure::CliMeasureDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ResourceId,
    pub measure: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliMeasureDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliMeasureDelete) -> Result<Self, Self::Error> {
        let CliMeasureDelete {
            project,
            measure,
            backend,
        } = delete;
        Ok(Self {
            project,
            measure,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: bencher_client::JsonUnit = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_measure_delete()
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
