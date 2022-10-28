use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonNewBranch, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::project::branch::CliBranchCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub name: String,
    pub slug: Option<String>,
    pub backend: Backend,
}

impl TryFrom<CliBranchCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliBranchCreate) -> Result<Self, Self::Error> {
        let CliBranchCreate {
            project,
            name,
            slug,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewBranch {
    fn from(create: Create) -> Self {
        let Create {
            project: _,
            name,
            slug,
            backend: _,
        } = create;
        Self { name, slug }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        let branch: JsonNewBranch = self.clone().into();
        self.backend
            .post(&format!("/v0/projects/{}/branches", self.project), &branch)
            .await?;
        Ok(())
    }
}
