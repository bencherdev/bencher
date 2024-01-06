use std::convert::TryFrom;

use bencher_client::types::JsonConfirm;
use bencher_json::Jwt;

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::system::auth::CliAuthConfirm,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Confirm {
    pub token: Jwt,
    pub backend: PubBackend,
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

impl From<Confirm> for JsonConfirm {
    fn from(confirm: Confirm) -> Self {
        let Confirm { token, .. } = confirm;
        Self {
            token: token.into(),
        }
    }
}

impl SubCmd for Confirm {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(
                |client| async move { client.auth_confirm_post().body(self.clone()).send().await },
            )
            .await?;
        Ok(())
    }
}
