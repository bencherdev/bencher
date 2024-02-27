use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::member::CliMemberView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub organization: ResourceId,
    pub user: ResourceId,
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
