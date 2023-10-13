#![cfg(feature = "plus")]

use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::organization::usage::JsonUsage;
use bencher_json::{DateTime, DateTimeMillis, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::organization::usage::CliOrganizationUsage,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Usage {
    pub organization: ResourceId,
    pub start: DateTime,
    pub end: DateTime,
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
        let _json: JsonUsage = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .org_usage_get()
                        .organization(self.organization.clone())
                        .start(DateTimeMillis::from(self.start))
                        .end(DateTimeMillis::from(self.end))
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
