use std::{convert::TryFrom, str::FromStr};

use async_trait::async_trait;
use bencher_json::{BranchName, JsonNewBranch, ResourceId, Slug};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::branch::CliBranchCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub name: BranchName,
    pub slug: Option<Slug>,
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
            name: BranchName::from_str(&name)?,
            slug: if let Some(slug) = slug {
                Some(Slug::from_str(&slug)?)
            } else {
                None
            },
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewBranch {
    fn from(create: Create) -> Self {
        let Create { name, slug, .. } = create;
        Self { name, slug }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let branch: JsonNewBranch = self.clone().into();
        self.backend
            .post(&format!("/v0/projects/{}/branches", self.project), &branch)
            .await?;
        Ok(())
    }
}
