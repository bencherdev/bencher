use std::convert::TryFrom;

use bencher_json::ResourceId;

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::CliProjectView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub backend: PubBackend,
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

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .project_get()
                    .project(self.project.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
