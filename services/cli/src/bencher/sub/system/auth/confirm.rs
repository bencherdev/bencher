use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{system::auth::JsonConfirm, JsonAuthToken};
use bencher_valid::is_valid_jwt;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::auth::CliAuthConfirm,
    CliError,
};

const CONFIRM_PATH: &str = "/v0/auth/confirm";

#[derive(Debug, Clone)]
pub struct Confirm {
    pub token: String,
    pub backend: Backend,
}

impl TryFrom<CliAuthConfirm> for Confirm {
    type Error = CliError;

    fn try_from(confirm: CliAuthConfirm) -> Result<Self, Self::Error> {
        let CliAuthConfirm { token, host } = confirm;
        if !is_valid_jwt(&token) {
            return Err(CliError::Jwt(token.clone()));
        }
        let backend = Backend::new(None, host)?;
        Ok(Self { token, backend })
    }
}

#[async_trait]
impl SubCmd for Confirm {
    async fn exec(&self) -> Result<(), CliError> {
        let json_token: JsonAuthToken = self.token.clone().into();
        let res = self.backend.post(CONFIRM_PATH, &json_token).await?;
        let _: JsonConfirm = serde_json::from_value(res)?;
        Ok(())
    }
}
