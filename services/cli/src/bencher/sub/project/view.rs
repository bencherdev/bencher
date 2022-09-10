use std::{convert::TryFrom, path::Path};

use async_trait::async_trait;
use bencher_json::ResourceId;

use super::PROJECTS_PATH;
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
        let path = Path::new(PROJECTS_PATH);
        let path = path.join(self.project.as_str());
        self.backend
            .get(path.to_str().unwrap_or(PROJECTS_PATH))
            .await?;
        Ok(())
    }
}
