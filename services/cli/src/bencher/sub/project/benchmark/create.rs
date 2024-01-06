use std::convert::TryFrom;

use bencher_client::types::JsonNewBenchmark;
use bencher_json::{BenchmarkName, ResourceId, Slug};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::benchmark::CliBenchmarkCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub name: BenchmarkName,
    pub slug: Option<Slug>,
    pub backend: AuthBackend,
}

impl TryFrom<CliBenchmarkCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliBenchmarkCreate) -> Result<Self, Self::Error> {
        let CliBenchmarkCreate {
            project,
            name,
            slug,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewBenchmark {
    fn from(create: Create) -> Self {
        let Create { name, slug, .. } = create;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
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
