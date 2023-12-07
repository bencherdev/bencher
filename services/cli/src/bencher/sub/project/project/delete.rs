use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::CliProjectDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliProjectDelete> for Delete {
    type Error = CliError;

    fn try_from(view: CliProjectDelete) -> Result<Self, Self::Error> {
        let CliProjectDelete { project, backend } = view;
        Ok(Self {
            project,
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
                        .project_delete()
                        .project(self.project.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
