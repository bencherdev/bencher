use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonEmpty, JsonLogin};

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::auth::CliAuthLogin,
    BencherError,
};

const LOGIN_PATH: &str = "/v0/auth/login";

#[derive(Debug, Clone)]
pub struct Login {
    pub email: String,
    pub backend: Backend,
}

impl TryFrom<CliAuthLogin> for Login {
    type Error = BencherError;

    fn try_from(login: CliAuthLogin) -> Result<Self, Self::Error> {
        let CliAuthLogin { email, host } = login;
        let backend = Backend::new(None, host)?;
        Ok(Self { email, backend })
    }
}

impl Into<JsonLogin> for Login {
    fn into(self) -> JsonLogin {
        let Self { email, backend: _ } = self;
        JsonLogin { email }
    }
}

#[async_trait]
impl SubCmd for Login {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        let json_login: JsonLogin = self.clone().into();
        let res = self.backend.post(LOGIN_PATH, &json_login).await?;
        let _: JsonEmpty = serde_json::from_value(res)?;
        Ok(())
    }
}
