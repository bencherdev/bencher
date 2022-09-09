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
    pub invite: Option<String>,
    pub backend: Backend,
}

impl TryFrom<CliAuthLogin> for Login {
    type Error = BencherError;

    fn try_from(login: CliAuthLogin) -> Result<Self, Self::Error> {
        let CliAuthLogin {
            email,
            invite,
            host,
        } = login;
        let backend = Backend::new(None, host)?;
        Ok(Self {
            email,
            invite,
            backend,
        })
    }
}

impl From<Login> for JsonLogin {
    fn from(login: Login) -> Self {
        let Login {
            email,
            invite,
            backend: _,
        } = login;
        Self {
            email,
            invite: invite.map(Into::into),
        }
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
