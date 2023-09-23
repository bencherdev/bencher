#![cfg(feature = "plus")]

use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::organization::usage::JsonUsage;
use bencher_json::ResourceId;
use chrono::{DateTime, Utc};

use crate::bencher::to_date_time;
use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::organization::usage::CliOrganizationUsage,
    CliError,
};

#[derive(Debug, Clone)]
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
            start: to_date_time(start)?,
            end: to_date_time(end)?,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Usage {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonUsage = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .org_usage_get()
                        .organization(self.organization.clone())
                        .start(self.start.timestamp())
                        .end(self.end.timestamp())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
