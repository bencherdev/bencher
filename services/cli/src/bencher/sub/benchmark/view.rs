use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;
use uuid::Uuid;

use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::benchmark::CliBenchmarkView,
    BencherError,
};

#[derive(Debug)]
pub struct View {
    pub project:   ResourceId,
    pub benchmark: Uuid,
    pub backend:   Backend,
}

impl TryFrom<CliBenchmarkView> for View {
    type Error = BencherError;

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
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        self.backend
            .get(&format!(
                "/v0/projects/{}/benchmarks/{}",
                self.project.as_str(),
                self.benchmark.to_string()
            ))
            .await?;
        Ok(())
    }
}
