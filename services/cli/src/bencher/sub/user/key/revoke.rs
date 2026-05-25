use bencher_json::{UserKeyUuid, UserResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::user::key::CliUserKeyRevoke,
};

#[derive(Debug)]
pub struct Revoke {
    pub user: UserResourceId,
    pub key: UserKeyUuid,
    pub backend: AuthBackend,
}

impl TryFrom<CliUserKeyRevoke> for Revoke {
    type Error = CliError;

    fn try_from(revoke: CliUserKeyRevoke) -> Result<Self, Self::Error> {
        let CliUserKeyRevoke {
            user,
            uuid: key,
            backend,
        } = revoke;
        Ok(Self {
            user,
            key,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Revoke {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .user_key_delete()
                    .user(self.user.clone())
                    .key(self.key)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
