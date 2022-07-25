use async_trait::async_trait;

use crate::{
    bencher::{
        sub::SubCmd,
        wide::Wide,
    },
    cli::CliAuth,
    BencherError,
};

mod signup;

use signup::Signup;

#[derive(Debug)]
pub enum Auth {
    Signup(Signup),
}

impl TryFrom<CliAuth> for Auth {
    type Error = BencherError;

    fn try_from(auth: CliAuth) -> Result<Self, Self::Error> {
        Ok(match auth {
            CliAuth::Signup(signup) => Self::Signup(signup.try_into()?),
        })
    }
}

#[async_trait]
impl SubCmd for Auth {
    async fn exec(&self, wide: &Wide) -> Result<(), BencherError> {
        match self {
            Self::Signup(signup) => signup.exec(wide).await,
        }
    }
}
