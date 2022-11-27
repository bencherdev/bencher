use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonEmpty, JsonSignup};
use bencher_valid::{is_valid_jwt, Email, UserName};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::auth::CliAuthSignup,
    CliError,
};

const SIGNUP_PATH: &str = "/v0/auth/signup";

#[derive(Debug, Clone)]
pub struct Signup {
    pub name: UserName,
    pub slug: Option<String>,
    pub email: Email,
    pub invite: Option<String>,
    pub backend: Backend,
}

impl TryFrom<CliAuthSignup> for Signup {
    type Error = CliError;

    fn try_from(signup: CliAuthSignup) -> Result<Self, Self::Error> {
        let CliAuthSignup {
            name,
            slug,
            email,
            invite,
            host,
        } = signup;
        if let Some(invite) = &invite {
            if !is_valid_jwt(invite) {
                return Err(CliError::Jwt(invite.clone()));
            }
        }
        Ok(Self {
            name: name.parse()?,
            slug,
            email: email.parse()?,
            invite,
            backend: Backend::new(None, host)?,
        })
    }
}

impl From<Signup> for JsonSignup {
    fn from(signup: Signup) -> Self {
        let Signup {
            name,
            slug,
            email,
            invite,
            backend: _,
        } = signup;
        Self {
            name,
            slug,
            email,
            invite: invite.map(Into::into),
        }
    }
}

#[async_trait]
impl SubCmd for Signup {
    async fn exec(&self) -> Result<(), CliError> {
        let json_signup: JsonSignup = self.clone().into();
        let res = self.backend.post(SIGNUP_PATH, &json_signup).await?;
        let _: JsonEmpty = serde_json::from_value(res)?;
        Ok(())
    }
}
