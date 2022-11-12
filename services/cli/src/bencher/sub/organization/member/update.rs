use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{
    organization::member::{JsonOrganizationRole, JsonUpdateMember},
    ResourceId,
};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::organization::member::CliMemberUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub org: ResourceId,
    pub user: ResourceId,
    pub role: Option<JsonOrganizationRole>,
    pub backend: Backend,
}

impl TryFrom<CliMemberUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliMemberUpdate) -> Result<Self, Self::Error> {
        let CliMemberUpdate {
            org,
            user,
            role,
            backend,
        } = update;
        Ok(Self {
            org,
            user,
            role: role.map(Into::into),
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateMember {
    fn from(update: Update) -> Self {
        Self { role: update.role }
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let update: JsonUpdateMember = self.clone().into();
        self.backend
            .patch(
                &format!("/v0/organizations/{}/members/{}", self.org, self.user),
                &update,
            )
            .await?;
        Ok(())
    }
}
