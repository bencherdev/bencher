use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::project::branch::CliBranchView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub branch: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliBranchView> for View {
    type Error = CliError;

    fn try_from(view: CliBranchView) -> Result<Self, Self::Error> {
        let CliBranchView {
            project,
            branch,
            backend,
        } = view;
        Ok(Self {
            project,
            branch,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend
            .get(&format!(
                "/v0/projects/{}/branches/{}",
                self.project, self.branch
            ))
            .await?;
        Ok(())
    }
}
