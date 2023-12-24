use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonNewToken;
use bencher_json::{ResourceId, ResourceName};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::user::token::CliTokenCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub user: ResourceId,
    pub name: ResourceName,
    pub ttl: Option<u32>,
    pub backend: Backend,
}

impl TryFrom<CliTokenCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliTokenCreate) -> Result<Self, Self::Error> {
        let CliTokenCreate {
            user,
            name,
            ttl,
            backend,
        } = create;
        Ok(Self {
            user,
            name,
            ttl,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewToken {
    fn from(create: Create) -> Self {
        let Create { name, ttl, .. } = create;
        Self {
            name: name.into(),
            ttl,
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .user_token_post()
                    .user(self.user.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
