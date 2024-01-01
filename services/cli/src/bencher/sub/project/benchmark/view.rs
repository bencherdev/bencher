use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::benchmark::CliBenchmarkView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub benchmark: ResourceId,
    pub backend: PubBackend,
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
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_benchmark_get()
                    .project(self.project.clone())
                    .benchmark(self.benchmark.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
