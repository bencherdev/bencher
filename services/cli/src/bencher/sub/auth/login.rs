use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{
    auth::JsonConfirmed,
    JsonLogin,
    JsonToken,
};

use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::auth::CliAuthLogin,
    BencherError,
};

const LOGIN_PATH: &str = "/v0/auth/login";

#[derive(Debug, Clone)]
pub struct Login {
    pub email:   String,
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

const CONFIRM_PATH: &str = "/v0/auth/confirm";
const BENCHER_TOKEN: &str = "BENCHER_TOKEN";

#[async_trait]
impl SubCmd for Login {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        use std::io::{
            stdin,
            stdout,
            Write,
        };

        let json_login: JsonLogin = self.clone().into();
        let res = self.backend.post(LOGIN_PATH, &json_login).await?;
        let _: () = serde_json::from_value(res)?;

        let mut token = String::new();
        print!("Please enter your confirmation token: ");
        let _ = stdout().flush();
        stdin().read_line(&mut token)?;
        token = token.trim().into();

        let json_token: JsonToken = token.into();
        let res = self.backend.post(CONFIRM_PATH, &json_token).await?;
        let _: JsonConfirmed = serde_json::from_value(res)?;

        Ok(())
    }
}
