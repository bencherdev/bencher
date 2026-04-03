use bencher_client::types::{BenchmarkName as ClientBenchmarkName, JsonNewBenchmark};
use bencher_json::{BenchmarkName, BenchmarkSlug, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::benchmark::CliBenchmarkCreate,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ProjectResourceId,
    pub name: BenchmarkName,
    pub slug: Option<BenchmarkSlug>,
    pub alias: Option<Vec<BenchmarkName>>,
    pub backend: AuthBackend,
}

impl TryFrom<CliBenchmarkCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliBenchmarkCreate) -> Result<Self, Self::Error> {
        let CliBenchmarkCreate {
            project,
            name,
            slug,
            alias,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug,
            alias,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewBenchmark {
    fn from(create: Create) -> Self {
        let Create {
            name, slug, alias, ..
        } = create;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
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

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_benchmark_post()
                    .project(self.project.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
