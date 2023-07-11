#![cfg(feature = "plus")]

use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{organization::usage::JsonUsage, ResourceId};
use chrono::serde::ts_milliseconds::deserialize as from_milli_ts;
use chrono::{DateTime, Utc};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::organization::usage::CliOrganizationUsage,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Usage {
    pub org: ResourceId,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub backend: Backend,
}

impl TryFrom<CliOrganizationUsage> for Usage {
    type Error = CliError;

    fn try_from(usage: CliOrganizationUsage) -> Result<Self, Self::Error> {
        let CliOrganizationUsage {
            org,
            start,
            end,
            backend,
        } = usage;

        Ok(Self {
            org,
            start: from_milli_ts(serde_json::json!(start))?,
            end: from_milli_ts(serde_json::json!(end))?,
            backend: backend.try_into()?,
        })
    }
}

impl From<Usage> for JsonUsage {
    fn from(usage: Usage) -> Self {
        let Usage { start, end, .. } = usage;
        Self { start, end }
    }
}

#[async_trait]
impl SubCmd for Usage {
    async fn exec(&self) -> Result<(), CliError> {
        let json_usage: JsonUsage = self.clone().into();
        self.backend
            .get_query(
                &format!("/v0/organizations/{}/usage", self.org),
                &json_usage,
            )
            .await?;
        Ok(())
    }
}
