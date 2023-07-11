use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonNewBranch, JsonStartPoint};
use bencher_json::{BranchName, ResourceId, Slug};

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
    pub soft: bool,
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
            soft,
            start_point_branch,
            start_point_thresholds,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug,
            soft,
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
            soft,
            start_point_branch,
            start_point_thresholds,
            ..
        } = create;
        let start_point = start_point_branch.map(|branch| JsonStartPoint {
            branch: branch.into(),
            thresholds: Some(start_point_thresholds),
        });
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
            soft: Some(soft),
            start_point,
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend
            .send_with(
                |client| async move {
                    client
                        .proj_branch_post()
                        .project(self.project.clone())
                        .body(self.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
