use bencher_json::{BenchmarkResourceId, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::benchmark::CliBenchmarkView,
};

#[derive(Debug)]
pub struct View {
    pub project: ProjectResourceId,
    pub benchmark: BenchmarkResourceId,
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
