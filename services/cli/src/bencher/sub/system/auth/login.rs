use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{Email, JsonEmpty, JsonLogin, Jwt, Plan};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::auth::CliAuthLogin,
    CliError,
};

const LOGIN_PATH: &str = "/v0/auth/login";

#[derive(Debug, Clone)]
pub struct Login {
    pub email: Email,
    pub plan: Option<Plan>,
    pub invite: Option<Jwt>,
    pub backend: Backend,
}

impl TryFrom<CliAuthLogin> for Login {
    type Error = CliError;

    fn try_from(login: CliAuthLogin) -> Result<Self, Self::Error> {
        let CliAuthLogin {
            email,
            plan,
            invite,
            backend,
        } = login;
        Ok(Self {
            email,
            plan,
            invite,
            backend: backend.try_into()?,
        })
    }
}

impl From<Login> for JsonLogin {
    fn from(login: Login) -> Self {
        let Login {
            email,
            plan,
            invite,
            ..
        } = login;
        Self {
            email,
            plan,
            invite,
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
