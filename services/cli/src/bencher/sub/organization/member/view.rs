use bencher_json::{OrganizationResourceId, UserResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::member::CliMemberView,
};

#[derive(Debug)]
pub struct View {
    pub organization: OrganizationResourceId,
    pub user: UserResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliMemberView> for View {
    type Error = CliError;

    fn try_from(view: CliMemberView) -> Result<Self, Self::Error> {
        let CliMemberView {
            organization,
            user,
            backend,
        } = view;
        Ok(Self {
            organization,
            user,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_member_get()
                    .organization(self.organization.clone())
                    .user(self.user.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
