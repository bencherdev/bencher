use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonUpdateToken;
use bencher_json::{ResourceId, ResourceName, TokenUuid};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::user::token::CliTokenUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub user: ResourceId,
    pub token: TokenUuid,
    pub name: Option<ResourceName>,
    pub backend: AuthBackend,
}

impl TryFrom<CliTokenUpdate> for Update {
    type Error = CliError;

    fn try_from(view: CliTokenUpdate) -> Result<Self, Self::Error> {
        let CliTokenUpdate {
            user,
            token,
            name,
            backend,
        } = view;
        Ok(Self {
            user,
            token,
            name,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateToken {
    fn from(update: Update) -> Self {
        let Update { name, .. } = update;
        Self {
            name: name.map(Into::into),
        }
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .as_ref()
            .send(|client| async move {
                client
                    .user_token_patch()
                    .user(self.user.clone())
                    .token(self.token)
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
