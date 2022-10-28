use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::project::project::CliProjectView,
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
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        let path = if let Some(org) = &self.org {
            format!("/v0/organizations/{}/projects/{}", org, self.project)
        } else {
            format!("/v0/projects/{}", self.project)
        };
        self.backend.get(&path).await?;
        Ok(())
    }
}
