use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonBenchmark, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::benchmark::CliBenchmarkView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub benchmark: ResourceId,
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
        let _json: JsonBenchmark = self
            .backend
            .send_with(|client| async move {
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
