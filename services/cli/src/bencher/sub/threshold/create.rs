use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonNewThreshold;
use uuid::Uuid;

use super::statistic::Statistic;
use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::threshold::CliThresholdCreate,
    BencherError,
};

const THRESHOLDS_PATH: &str = "/v0/thresholds";

#[derive(Debug)]
pub struct Create {
    pub branch:    Uuid,
    pub testbed:   Uuid,
    pub statistic: Statistic,
    pub backend:   Backend,
}

impl TryFrom<CliThresholdCreate> for Create {
    type Error = BencherError;

    fn try_from(create: CliThresholdCreate) -> Result<Self, Self::Error> {
        let CliThresholdCreate {
            branch,
            testbed,
            statistic,
            backend,
        } = create;
        Ok(Self {
            branch,
            testbed,
            statistic: statistic.try_into()?,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        let threshold = JsonNewThreshold {
            branch:    self.branch.clone(),
            testbed:   self.testbed.clone(),
            statistic: self.statistic.into(),
        };
        self.backend.post(THRESHOLDS_PATH, &threshold).await?;
        Ok(())
    }
}
