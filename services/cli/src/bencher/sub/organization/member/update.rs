use bencher_client::types::{JsonUpdateMember, OrganizationRole};
use bencher_json::{OrganizationResourceId, UserResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::member::CliMemberUpdate,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub organization: OrganizationResourceId,
    pub user: UserResourceId,
    pub role: Option<OrganizationRole>,
    pub backend: AuthBackend,
}

impl TryFrom<CliMemberUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliMemberUpdate) -> Result<Self, Self::Error> {
        let CliMemberUpdate {
            organization,
            user,
            role,
            backend,
        } = update;
        Ok(Self {
            organization,
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
                    .organization(self.organization.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
