use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::project::CliProjectView,
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
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend
            .get(&format!("/v0/projects/{}", self.project))
            .await?;
        Ok(())
    }
}
