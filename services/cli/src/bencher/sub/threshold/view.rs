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
    cli::threshold::CliThresholdView,
    BencherError,
};

#[derive(Debug)]
pub struct View {
    pub project:   ResourceId,
    pub threshold: Uuid,
    pub backend:   Backend,
}

impl TryFrom<CliThresholdView> for View {
    type Error = BencherError;

    fn try_from(view: CliThresholdView) -> Result<Self, Self::Error> {
        let CliThresholdView {
            project,
            threshold,
            backend,
        } = view;
        Ok(Self {
            project,
            threshold,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        self.backend
            .get(&format!(
                "/v0/projects/{}/thresholds/{}",
                self.project.as_str(),
                self.threshold.to_string()
            ))
            .await?;
        Ok(())
    }
}
