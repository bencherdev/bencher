use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonUpdateBenchmark;
use bencher_json::{BenchmarkName, JsonBenchmark, ResourceId, Slug};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::benchmark::CliBenchmarkUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub benchmark: ResourceId,
    pub name: Option<BenchmarkName>,
    pub slug: Option<Slug>,
    pub backend: Backend,
}

impl TryFrom<CliBenchmarkUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliBenchmarkUpdate) -> Result<Self, Self::Error> {
        let CliBenchmarkUpdate {
            project,
            benchmark,
            name,
            slug,
            backend,
        } = create;
        Ok(Self {
            project,
            benchmark,
            name,
            slug,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateBenchmark {
    fn from(create: Update) -> Self {
        let Update { name, slug, .. } = create;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
        }
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonBenchmark = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_benchmark_patch()
                        .project(self.project.clone())
                        .benchmark(self.benchmark.clone())
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
