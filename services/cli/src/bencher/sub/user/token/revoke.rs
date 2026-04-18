use bencher_json::{TokenUuid, UserResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::user::token::CliTokenRevoke,
};

#[derive(Debug)]
pub struct Revoke {
    pub user: UserResourceId,
    pub token: TokenUuid,
    pub backend: AuthBackend,
}

impl TryFrom<CliTokenRevoke> for Revoke {
    type Error = CliError;

    fn try_from(revoke: CliTokenRevoke) -> Result<Self, Self::Error> {
        let CliTokenRevoke {
            user,
            uuid: token,
            backend,
        } = revoke;
        Ok(Self {
            user,
            token,
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
                    .user_token_delete()
                    .user(self.user.clone())
                    .token(self.token)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
