use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{Email, JsonEmpty, JsonLogin, Jwt};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::auth::CliAuthLogin,
    CliError,
};

const LOGIN_PATH: &str = "/v0/auth/login";

#[derive(Debug, Clone)]
pub struct Login {
    pub email: Email,
    pub invite: Option<Jwt>,
    pub backend: Backend,
}

impl TryFrom<CliAuthLogin> for Login {
    type Error = CliError;

    fn try_from(login: CliAuthLogin) -> Result<Self, Self::Error> {
        let CliAuthLogin {
            email,
            invite,
            backend,
        } = login;
        Ok(Self {
            email: email.parse()?,
            invite: if let Some(invite) = invite {
                Some(invite.parse()?)
            } else {
                None
            },
            backend: backend.try_into()?,
        })
    }
}

impl From<Login> for JsonLogin {
    fn from(login: Login) -> Self {
        let Login { email, invite, .. } = login;
        Self { email, invite }
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
