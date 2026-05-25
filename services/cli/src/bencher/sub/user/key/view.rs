use bencher_json::{UserKeyUuid, UserResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::user::key::CliUserKeyView,
};

#[derive(Debug)]
pub struct View {
    pub user: UserResourceId,
    pub key: UserKeyUuid,
    pub backend: AuthBackend,
}

impl TryFrom<CliUserKeyView> for View {
    type Error = CliError;

    fn try_from(view: CliUserKeyView) -> Result<Self, Self::Error> {
        let CliUserKeyView {
            user,
            uuid: key,
            backend,
        } = view;
        Ok(Self {
            user,
            key,
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
                    .user_key_get()
                    .user(self.user.clone())
                    .key(self.key)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
