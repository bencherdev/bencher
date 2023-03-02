#![cfg(feature = "plus")]

use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;
use chrono::{DateTime, Utc};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::organization::usage::CliOrganizationUsage,
    CliError,
};

#[derive(Debug)]
pub struct Usage {
    pub organization: ResourceId,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub backend: Backend,
}

impl TryFrom<CliOrganizationUsage> for Usage {
    type Error = CliError;

    fn try_from(usage: CliOrganizationUsage) -> Result<Self, Self::Error> {
        let CliOrganizationUsage {
            organization,
            start,
            end,
            backend,
        } = usage;
        Ok(Self {
            organization,
            start,
            end,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Usage {
    async fn exec(&self) -> Result<(), CliError> {
        let query = vec![
            (
                "start".to_string(),
                self.start.timestamp_millis().to_string(),
            ),
            ("end".to_string(), self.end.timestamp_millis().to_string()),
        ];
        self.backend
            .get_query(
                &format!("/v0/organizations/{}/usage", self.organization),
                &query,
            )
            .await?;
        Ok(())
    }
}
