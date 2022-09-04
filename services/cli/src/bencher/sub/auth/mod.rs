use async_trait::async_trait;

use crate::{
    bencher::{
        sub::SubCmd,
        wide::Wide,
    },
    cli::auth::CliAuth,
    BencherError,
};

mod confirm;
mod login;
mod signup;

use confirm::Confirm;
use login::Login;
use signup::Signup;

#[derive(Debug)]
pub enum Auth {
    Signup(Signup),
    Login(Login),
    Confirm(Confirm),
}

impl TryFrom<CliAuth> for Auth {
    type Error = BencherError;

    fn try_from(auth: CliAuth) -> Result<Self, Self::Error> {
        Ok(match auth {
            CliAuth::Signup(signup) => Self::Signup(signup.try_into()?),
            CliAuth::Login(login) => Self::Login(login.try_into()?),
            CliAuth::Confirm(confirm) => Self::Confirm(confirm.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Auth {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Self::Signup(signup) => signup.exec(wide).await,
            Self::Login(login) => login.exec(wide).await,
            Self::Confirm(confirm) => confirm.exec(wide).await,
        }
    }
}
