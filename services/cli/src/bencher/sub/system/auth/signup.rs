use std::{convert::TryFrom, str::FromStr};

use async_trait::async_trait;
use bencher_json::{Email, JsonEmpty, JsonSignup, Jwt, Slug, UserName};

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
            invite,
            backend,
        } = signup;
        Ok(Self {
            name: name.parse()?,
            slug: if let Some(slug) = slug {
                Some(Slug::from_str(&slug)?)
            } else {
                None
            },
            email: email.parse()?,
            invite: if let Some(invite) = invite {
                Some(invite.parse()?)
            } else {
                None
            },
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
            invite,
            ..
        } = signup;
        Self {
            name,
            slug,
            email,
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
