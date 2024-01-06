use std::convert::TryFrom;

use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::benchmark::CliBenchmarkDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ResourceId,
    pub benchmark: ResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliBenchmarkDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliBenchmarkDelete) -> Result<Self, Self::Error> {
        let CliBenchmarkDelete {
            project,
            benchmark,
            backend,
        } = delete;
        Ok(Self {
            project,
            benchmark,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_benchmark_delete()
                    .project(self.project.clone())
                    .benchmark(self.benchmark.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
