use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonEmpty, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::benchmark::CliBenchmarkDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ResourceId,
    pub benchmark: ResourceId,
    pub backend: Backend,
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

#[async_trait]
impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonEmpty = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_benchmark_delete()
                        .project(self.project.clone())
                        .benchmark(self.benchmark.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
