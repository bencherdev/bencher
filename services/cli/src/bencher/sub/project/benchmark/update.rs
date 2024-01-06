use std::convert::TryFrom;

use bencher_client::types::JsonUpdateBenchmark;
use bencher_json::{BenchmarkName, ResourceId, Slug};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::benchmark::CliBenchmarkUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub benchmark: ResourceId,
    pub name: Option<BenchmarkName>,
    pub slug: Option<Slug>,
    pub backend: AuthBackend,
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
    fn from(update: Update) -> Self {
        let Update { name, slug, .. } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_benchmark_patch()
                    .project(self.project.clone())
                    .benchmark(self.benchmark.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
