use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonNewThreshold;
use uuid::Uuid;

use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::threshold::CliThresholdCreate,
    BencherError,
};

const THESHOLDS_PATH: &str = "/v0/thresholds";

#[derive(Debug)]
pub struct Create {
    pub branch:  Uuid,
    pub testbed: Uuid,
    pub backend: Backend,
}

impl TryFrom<CliThresholdCreate> for Create {
    type Error = BencherError;

    fn try_from(create: CliThresholdCreate) -> Result<Self, Self::Error> {
        let CliThresholdCreate {
            branch,
            testbed,
            backend,
        } = create;
        Ok(Self {
            branch,
            testbed,
            backend: Backend::try_from(backend)?,
        })
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        let threshold = JsonNewThreshold {
            branch:  self.branch.clone(),
            testbed: self.testbed.clone(),
        };
        self.backend.post(THESHOLDS_PATH, &threshold).await?;
        Ok(())
    }
}
