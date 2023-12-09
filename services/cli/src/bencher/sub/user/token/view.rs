use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonToken, ResourceId, TokenUuid};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::user::token::CliTokenView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub user: ResourceId,
    pub token: TokenUuid,
    pub backend: Backend,
}

impl TryFrom<CliTokenView> for View {
    type Error = CliError;

    fn try_from(view: CliTokenView) -> Result<Self, Self::Error> {
        let CliTokenView {
            user,
            token,
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
        let _json: JsonToken = self
            .backend
            .send_with(|client| async move {
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
