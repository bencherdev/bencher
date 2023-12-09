use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonAuthToken;
use bencher_json::{JsonAuthUser, Jwt};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::system::auth::CliAuthConfirm,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Confirm {
    pub token: Jwt,
    pub backend: Backend,
}

impl TryFrom<CliAuthConfirm> for Confirm {
    type Error = CliError;

    fn try_from(confirm: CliAuthConfirm) -> Result<Self, Self::Error> {
        let CliAuthConfirm {
            confirm: token,
            backend,
        } = confirm;
        Ok(Self {
            token,
            backend: backend.try_into()?,
        })
    }
}

impl From<Confirm> for JsonAuthToken {
    fn from(confirm: Confirm) -> Self {
        let Confirm { token, .. } = confirm;
        Self {
            token: token.into(),
        }
    }
}

#[async_trait]
impl SubCmd for Confirm {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonAuthUser = self
            .backend
            .send_with(|client| async move {
                client.auth_confirm_post().body(self.clone()).send().await
            })
            .await?;
        Ok(())
    }
}
