use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::result::CliResultView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub result: Uuid,
    pub backend: Backend,
}

impl TryFrom<CliResultView> for View {
    type Error = CliError;

    fn try_from(view: CliResultView) -> Result<Self, Self::Error> {
        let CliResultView {
            project,
            result,
            backend,
        } = view;
        Ok(Self {
            project,
            result,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend
            .get(&format!(
                "/v0/projects/{}/results/{}",
                self.project, self.result
            ))
            .await?;
        Ok(())
    }
}
