use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonEmpty, JsonLogin};
use bencher_valid::{is_valid_jwt, Email};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::auth::CliAuthLogin,
    CliError,
};

const LOGIN_PATH: &str = "/v0/auth/login";

#[derive(Debug, Clone)]
pub struct Login {
    pub email: Email,
    pub invite: Option<String>,
    pub backend: Backend,
}

impl TryFrom<CliAuthLogin> for Login {
    type Error = CliError;

    fn try_from(login: CliAuthLogin) -> Result<Self, Self::Error> {
        let CliAuthLogin {
            email,
            invite,
            host,
        } = login;
        if let Some(invite) = &invite {
            if !is_valid_jwt(invite) {
                return Err(CliError::Jwt(invite.clone()));
            }
        }
        Ok(Self {
            email: email.parse()?,
            invite,
            backend: Backend::new(None, host)?,
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
    async fn exec(&self) -> Result<(), CliError> {
        let json_login: JsonLogin = self.clone().into();
        let res = self.backend.post(LOGIN_PATH, &json_login).await?;
        let _: JsonEmpty = serde_json::from_value(res)?;
        Ok(())
    }
}
