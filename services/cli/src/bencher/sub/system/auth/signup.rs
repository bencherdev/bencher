use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{Email, JsonEmpty, JsonSignup, Jwt, Plan, Slug, UserName};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::auth::CliAuthSignup,
    CliError,
};

const SIGNUP_PATH: &str = "/v0/auth/signup";

#[derive(Debug, Clone)]
pub struct Signup {
    pub name: UserName,
    pub slug: Option<Slug>,
    pub email: Email,
    pub plan: Option<Plan>,
    pub invite: Option<Jwt>,
    pub backend: Backend,
}

impl TryFrom<CliAuthSignup> for Signup {
    type Error = CliError;

    fn try_from(signup: CliAuthSignup) -> Result<Self, Self::Error> {
        let CliAuthSignup {
            name,
            slug,
            email,
            plan,
            invite,
            backend,
        } = signup;
        Ok(Self {
            name,
            slug,
            email,
            plan,
            invite,
            backend: backend.try_into()?,
        })
    }
}

impl From<Signup> for JsonSignup {
    fn from(signup: Signup) -> Self {
        let Signup {
            name,
            slug,
            email,
            plan,
            invite,
            ..
        } = signup;
        Self {
            name,
            slug,
            email,
            plan,
            invite,
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
