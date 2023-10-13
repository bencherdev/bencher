use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonUpdateMember, OrganizationRole};
use bencher_json::{JsonMember, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::organization::member::CliMemberUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub org: ResourceId,
    pub user: ResourceId,
    pub role: Option<OrganizationRole>,
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
        let _json: JsonMember = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .org_member_patch()
                        .organization(self.org.clone())
                        .body(self.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
