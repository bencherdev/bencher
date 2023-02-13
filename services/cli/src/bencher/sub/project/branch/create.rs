use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{project::branch::JsonStartPoint, BranchName, JsonNewBranch, ResourceId, Slug};

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
    pub start_point_branch: Option<ResourceId>,
    pub start_point_thresholds: bool,
    pub backend: Backend,
}

impl TryFrom<CliBranchCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliBranchCreate) -> Result<Self, Self::Error> {
        let CliBranchCreate {
            project,
            name,
            slug,
            start_point_branch,
            start_point_thresholds,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug,
            start_point_branch,
            start_point_thresholds,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewBranch {
    fn from(create: Create) -> Self {
        let Create {
            name,
            slug,
            start_point_branch,
            start_point_thresholds,
            ..
        } = create;
        let start_point = start_point_branch.map(|branch| JsonStartPoint {
            branch,
            thresholds: Some(start_point_thresholds),
        });
        Self {
            name,
            slug,
            start_point,
        }
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
