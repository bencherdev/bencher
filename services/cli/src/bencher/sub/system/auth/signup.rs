use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonEmpty, JsonSignup};
use bencher_valid::{is_valid_email, is_valid_jwt, is_valid_user_name};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::auth::CliAuthSignup,
    CliError,
};

const SIGNUP_PATH: &str = "/v0/auth/signup";

#[derive(Debug, Clone)]
pub struct Signup {
    pub name: String,
    pub slug: Option<String>,
    pub email: String,
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
        if !is_valid_user_name(&name) {
            return Err(CliError::UserName(name));
        }
        if !is_valid_email(&email) {
            return Err(CliError::Email(email));
        }
        if let Some(invite) = &invite {
            if !is_valid_jwt(invite) {
                return Err(CliError::Jwt(invite.clone()));
            }
        }
        let backend = Backend::new(None, host)?;
        Ok(Self {
            name,
            slug,
            email,
            invite,
            backend,
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
            ..
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
