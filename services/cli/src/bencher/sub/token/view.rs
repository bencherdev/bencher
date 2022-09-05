use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::token::CliTokenView,
    BencherError,
};

#[derive(Debug)]
pub struct View {
    pub user: ResourceId,
    pub token: Uuid,
    pub backend: Backend,
}

impl TryFrom<CliTokenView> for View {
    type Error = BencherError;

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
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        self.backend
            .get(&format!(
                "/v0/users/{}/tokens/{}",
                self.user.as_str(),
                self.token.to_string()
            ))
            .await?;
        Ok(())
    }
}
