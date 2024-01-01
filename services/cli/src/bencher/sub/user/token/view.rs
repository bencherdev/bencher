use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{ResourceId, TokenUuid};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::user::token::CliTokenView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub user: ResourceId,
    pub token: TokenUuid,
    pub backend: AuthBackend,
}

impl TryFrom<CliTokenView> for View {
    type Error = CliError;

    fn try_from(view: CliTokenView) -> Result<Self, Self::Error> {
        let CliTokenView {
            user,
            uuid: token,
            backend,
        } = view;
        Ok(Self {
            user,
            token,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .user_token_get()
                    .user(self.user.clone())
                    .token(self.token)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
