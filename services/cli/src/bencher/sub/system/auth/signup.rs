use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonEmpty, JsonSignup};
use email_address_parser::EmailAddress;

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
    pub email: EmailAddress,
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
        let backend = Backend::new(None, host)?;
        Ok(Self {
            name,
            slug,
            email: EmailAddress::parse(&email, None).ok_or(CliError::Email(email))?,
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
            backend: _,
        } = signup;
        Self {
            name,
            slug,
            email: email.to_string(),
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
