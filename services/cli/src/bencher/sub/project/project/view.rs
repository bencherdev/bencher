use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::CliProjectView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub org: Option<ResourceId>,
    pub project: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliProjectView> for View {
    type Error = CliError;

    fn try_from(view: CliProjectView) -> Result<Self, Self::Error> {
        let CliProjectView {
            org,
            project,
            backend,
        } = view;
        Ok(Self {
            org,
            project,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend
            .send_with(
                |client| async move {
                    if let Some(org) = self.org.clone() {
                        client
                            .org_project_get()
                            .organization(org)
                            .project(self.project.clone())
                            .send()
                            .await
                    } else {
                        client
                            .project_get()
                            .project(self.project.clone())
                            .send()
                            .await
                    }
                },
                true,
            )
            .await?;
        Ok(())
    }
}
