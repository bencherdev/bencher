use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonLogin;
use bencher_json::{Email, JsonEmpty, Jwt};

#[cfg(feature = "plus")]
use crate::bencher::sub::organization::organization::plan::level::Level;
use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::system::auth::CliAuthLogin,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Login {
    pub email: Email,
    #[cfg(feature = "plus")]
    pub plan: Option<Level>,
    pub invite: Option<Jwt>,
    pub backend: Backend,
}

impl TryFrom<CliAuthLogin> for Login {
    type Error = CliError;

    fn try_from(login: CliAuthLogin) -> Result<Self, Self::Error> {
        let CliAuthLogin {
            email,
            #[cfg(feature = "plus")]
            plan,
            invite,
            backend,
        } = login;
        Ok(Self {
            email,
            #[cfg(feature = "plus")]
            plan: plan.map(Into::into),
            invite,
            backend: backend.try_into()?,
        })
    }
}

impl From<Login> for JsonLogin {
    fn from(login: Login) -> Self {
        let Login {
            email,
            #[cfg(feature = "plus")]
            plan,
            invite,
            ..
        } = login;
        Self {
            email: email.into(),
            #[cfg(feature = "plus")]
            plan: plan.map(Into::into),
            #[cfg(not(feature = "plus"))]
            plan: None,
            invite: invite.map(Into::into),
        }
    }
}

#[async_trait]
impl SubCmd for Login {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonEmpty = self
            .backend
            .send_with(
                |client| async move { client.auth_login_post().body(self.clone()).send().await },
                true,
            )
            .await?;
        Ok(())
    }
}
