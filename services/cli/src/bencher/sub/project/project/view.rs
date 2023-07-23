use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonProject, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::CliProjectView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliProjectView> for View {
    type Error = CliError;

    fn try_from(view: CliProjectView) -> Result<Self, Self::Error> {
        let CliProjectView { project, backend } = view;
        Ok(Self {
            project,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonProject = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .project_get()
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
