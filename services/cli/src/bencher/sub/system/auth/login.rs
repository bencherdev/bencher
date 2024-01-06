use std::convert::TryFrom;

use bencher_client::types::JsonLogin;
#[cfg(feature = "plus")]
use bencher_client::types::PlanLevel;
use bencher_json::{Email, Jwt};

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::system::auth::CliAuthLogin,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Login {
    pub email: Email,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    pub invite: Option<Jwt>,
    pub backend: PubBackend,
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
            plan,
            #[cfg(not(feature = "plus"))]
            plan: None,
            invite: invite.map(Into::into),
        }
    }
}

impl SubCmd for Login {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.auth_login_post().body(self.clone()).send().await })
            .await?;
        Ok(())
    }
}
