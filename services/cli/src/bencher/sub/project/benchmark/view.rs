use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::benchmark::CliBenchmarkView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub benchmark: Uuid,
    pub backend: Backend,
}

impl TryFrom<CliBenchmarkView> for View {
    type Error = CliError;

    fn try_from(view: CliBenchmarkView) -> Result<Self, Self::Error> {
        let CliBenchmarkView {
            project,
            benchmark,
            backend,
        } = view;
        Ok(Self {
            project,
            benchmark,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend
            .get(&format!(
                "/v0/projects/{}/benchmarks/{}",
                self.project, self.benchmark
            ))
            .await?;
        Ok(())
    }
}
