use bencher_client::types::{BenchmarkName as ClientBenchmarkName, JsonUpdateBenchmark};
use bencher_json::{BenchmarkName, BenchmarkResourceId, BenchmarkSlug, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::benchmark::CliBenchmarkUpdate,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ProjectResourceId,
    pub benchmark: BenchmarkResourceId,
    pub name: Option<BenchmarkName>,
    pub slug: Option<BenchmarkSlug>,
    pub archived: Option<bool>,
    pub alias: Option<Vec<BenchmarkName>>,
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
            alias,
            archived,
            backend,
        } = create;
        Ok(Self {
            project,
            benchmark,
            name,
            slug,
            archived: archived.into(),
            alias,
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
            alias,
            ..
        } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            archived,
            aliases: alias.and_then(|values| {
                if values.is_empty() {
                    None
                } else {
                    Some(
                        values
                            .into_iter()
                            .map(|a| ClientBenchmarkName(String::from(a.as_ref())))
                            .collect(),
                    )
                }
            }),
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
