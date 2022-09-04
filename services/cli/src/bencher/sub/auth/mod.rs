use std::io::{
    stdin,
    stdout,
    Write,
};

use async_trait::async_trait;
use bencher_json::{
    auth::JsonConfirmed,
    JsonToken,
};

use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::auth::CliAuth,
    BencherError,
};

mod login;
mod signup;

use login::Login;
use signup::Signup;

const CONFIRM_PATH: &str = "/v0/auth/confirm";

#[derive(Debug)]
pub enum Auth {
    Signup(Signup),
    Login(Login),
}

impl TryFrom<CliAuth> for Auth {
    type Error = BencherError;

    fn try_from(auth: CliAuth) -> Result<Self, Self::Error> {
        Ok(match auth {
            CliAuth::Signup(signup) => Self::Signup(signup.try_into()?),
            CliAuth::Login(login) => Self::Login(login.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Auth {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Self::Signup(signup) => signup.exec(wide).await,
            Self::Login(login) => login.exec(wide).await,
        }
    }
}

async fn confirm(backend: &Backend) -> Result<(), BencherError> {
    let mut token = String::new();
    print!("Please enter your confirmation token: ");
    let _ = stdout().flush();
    stdin().read_line(&mut token)?;
    token = token.trim().into();

    let json_token: JsonToken = token.into();
    let res = backend.post(CONFIRM_PATH, &json_token).await?;
    let _: JsonConfirmed = serde_json::from_value(res)?;

    Ok(())
}
