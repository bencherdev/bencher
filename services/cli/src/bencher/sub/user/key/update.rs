use bencher_client::types::JsonUpdateUserKey;
use bencher_json::{ResourceName, UserKeyUuid, UserResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::user::key::CliUserKeyUpdate,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub user: UserResourceId,
    pub key: UserKeyUuid,
    pub name: Option<ResourceName>,
    pub backend: AuthBackend,
}

impl TryFrom<CliUserKeyUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliUserKeyUpdate) -> Result<Self, Self::Error> {
        let CliUserKeyUpdate {
            user,
            uuid: key,
            name,
            backend,
        } = update;
        Ok(Self {
            user,
            key,
            name,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateUserKey {
    fn from(update: Update) -> Self {
        let Update { name, .. } = update;
        Self {
            name: name.map(Into::into),
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .user_key_patch()
                    .user(self.user.clone())
                    .key(self.key)
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
