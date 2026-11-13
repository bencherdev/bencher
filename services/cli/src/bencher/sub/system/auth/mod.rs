use crate::{CliError, bencher::sub::SubCmd, parser::system::auth::CliAuth};

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
    type Error = CliError;

    fn try_from(auth: CliAuth) -> Result<Self, Self::Error> {
        Ok(match auth {
            CliAuth::Signup(signup) => Self::Signup(signup.try_into()?),
            CliAuth::Login(login) => Self::Login(login.try_into()?),
            CliAuth::Confirm(confirm) => Self::Confirm(confirm.try_into()?),
        })
    }
}

impl SubCmd for Auth {
    async fn exec(&self) -> Result<(), CliError> {
        match self {
            Self::Signup(signup) => signup.exec().await,
            Self::Login(login) => login.exec().await,
            Self::Confirm(confirm) => confirm.exec().await,
        }
    }
}
