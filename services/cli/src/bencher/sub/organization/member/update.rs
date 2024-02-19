use bencher_client::types::{JsonUpdateMember, OrganizationRole};
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::member::CliMemberUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub org: ResourceId,
    pub user: ResourceId,
    pub role: Option<OrganizationRole>,
    pub backend: AuthBackend,
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

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_member_patch()
                    .organization(self.org.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
