use std::convert::TryFrom;

use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::plan::CliPlanDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub organization: ResourceId,
    pub remote: Option<bool>,
    pub backend: AuthBackend,
}

impl TryFrom<CliPlanDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliPlanDelete) -> Result<Self, Self::Error> {
        let CliPlanDelete {
            organization,
            skip_remote,
            backend,
        } = delete;
        Ok(Self {
            organization,
            remote: skip_remote.then_some(false),
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                let mut client = client
                    .org_plan_delete()
                    .organization(self.organization.clone());

                if let Some(remote) = self.remote {
                    client = client.remote(remote);
                }

                client.send().await
            })
            .await?;
        Ok(())
    }
}
