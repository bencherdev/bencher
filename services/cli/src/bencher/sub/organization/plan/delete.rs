use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::plan::CliPlanDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub organization: ResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliPlanDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliPlanDelete) -> Result<Self, Self::Error> {
        let CliPlanDelete {
            organization,
            backend,
        } = delete;
        Ok(Self {
            organization,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .as_ref()
            .send(|client| async move {
                client
                    .org_plan_delete()
                    .organization(self.organization.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
