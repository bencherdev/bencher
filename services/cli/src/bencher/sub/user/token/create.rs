use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonNewToken, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::user::token::CliTokenCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub user: ResourceId,
    pub name: String,
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
        Self { name, ttl }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let token: JsonNewToken = self.clone().into();
        self.backend
            .post(&format!("/v0/users/{}/tokens", self.user), &token)
            .await?;
        Ok(())
    }
}
