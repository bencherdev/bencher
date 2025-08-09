use bencher_client::types::JsonUpdateBenchmark;
use bencher_json::{BenchmarkName, BenchmarkSlug, ResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::benchmark::CliBenchmarkUpdate,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub benchmark: ResourceId,
    pub name: Option<BenchmarkName>,
    pub slug: Option<BenchmarkSlug>,
    pub archived: Option<bool>,
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
            archived,
            backend,
        } = create;
        Ok(Self {
            project,
            benchmark,
            name,
            slug,
            archived: archived.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateBenchmark {
    fn from(update: Update) -> Self {
        let Update {
            name,
            slug,
            archived,
            ..
        } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            archived,
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
