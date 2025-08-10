use bencher_json::{OrganizationResourceId, UserResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::member::CliMemberRemove,
};

#[derive(Debug)]
pub struct Remove {
    pub organization: OrganizationResourceId,
    pub user: UserResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliMemberRemove> for Remove {
    type Error = CliError;

    fn try_from(remove: CliMemberRemove) -> Result<Self, Self::Error> {
        let CliMemberRemove {
            organization,
            user,
            backend,
        } = remove;
        Ok(Self {
            organization,
            user,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Remove {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_member_delete()
                    .organization(self.organization.clone())
                    .user(self.user.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
