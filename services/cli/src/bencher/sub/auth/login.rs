use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonLogin;

use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::CliAuthLogin,
    BencherError,
};

const LOGIN_PATH: &str = "/v0/auth/login";

#[derive(Debug)]
pub struct Login {
    pub email:   String,
    pub backend: Backend,
}

impl TryFrom<CliAuthLogin> for Login {
    type Error = BencherError;

    fn try_from(login: CliAuthLogin) -> Result<Self, Self::Error> {
        let CliAuthLogin { email, url } = login;
        let backend = Backend::new(None, url)?;
        Ok(Self { email, backend })
    }
}

#[async_trait]
impl SubCmd for Login {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        let login = JsonLogin {
            email: self.email.clone(),
        };
        self.backend.post(LOGIN_PATH, &login).await?;
        Ok(())
    }
}
