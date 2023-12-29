use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::member::CliMemberView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub org: ResourceId,
    pub user: ResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliMemberView> for View {
    type Error = CliError;

    fn try_from(view: CliMemberView) -> Result<Self, Self::Error> {
        let CliMemberView { org, user, backend } = view;
        Ok(Self {
            org,
            user,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .as_ref()
            .send(|client| async move {
                client
                    .org_member_get()
                    .organization(self.org.clone())
                    .user(self.user.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
