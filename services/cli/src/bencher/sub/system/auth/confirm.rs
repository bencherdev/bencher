use std::{convert::TryFrom, str::FromStr};

use async_trait::async_trait;
use bencher_json::{system::auth::JsonConfirm, JsonAuthToken, Jwt};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::auth::CliAuthConfirm,
    CliError,
};

const CONFIRM_PATH: &str = "/v0/auth/confirm";

#[derive(Debug, Clone)]
pub struct Confirm {
    pub token: Jwt,
    pub backend: Backend,
}

impl TryFrom<CliAuthConfirm> for Confirm {
    type Error = CliError;

    fn try_from(confirm: CliAuthConfirm) -> Result<Self, Self::Error> {
        let CliAuthConfirm { token, backend } = confirm;
        Ok(Self {
            token: Jwt::from_str(&token)?,
            backend: backend.try_into()?,
        })
    }
}

impl From<Confirm> for JsonAuthToken {
    fn from(confirm: Confirm) -> Self {
        let Confirm { token, backend: _ } = confirm;
        Self { token }
    }
}

#[async_trait]
impl SubCmd for Confirm {
    async fn exec(&self) -> Result<(), CliError> {
        let json_token: JsonAuthToken = self.clone().into();
        let res = self.backend.post(CONFIRM_PATH, &json_token).await?;
        let _: JsonConfirm = serde_json::from_value(res)?;
        Ok(())
    }
}
