use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::token::CliTokenView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub user: ResourceId,
    pub uuid: Uuid,
    pub backend: Backend,
}

impl TryFrom<CliTokenView> for View {
    type Error = CliError;

    fn try_from(view: CliTokenView) -> Result<Self, Self::Error> {
        let CliTokenView {
            user,
            uuid,
            backend,
        } = view;
        Ok(Self {
            user,
            uuid,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend
            .get(&format!("/v0/users/{}/tokens/{}", self.user, self.uuid))
            .await?;
        Ok(())
    }
}
