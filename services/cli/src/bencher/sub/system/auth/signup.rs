use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonSignup;
use bencher_json::{Email, JsonEmpty, Jwt, Slug, UserName};

#[cfg(feature = "plus")]
use crate::bencher::sub::organization::resource::plan::level::Level;
use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::system::auth::CliAuthSignup,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Signup {
    pub name: UserName,
    pub slug: Option<Slug>,
    pub email: Email,
    #[cfg(feature = "plus")]
    pub plan: Option<Level>,
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
            #[cfg(feature = "plus")]
            plan,
            invite,
            backend,
        } = signup;
        Ok(Self {
            name,
            slug,
            email,
            #[cfg(feature = "plus")]
            plan: plan.map(Into::into),
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
            #[cfg(feature = "plus")]
            plan,
            invite,
            ..
        } = signup;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
            email: email.into(),
            #[cfg(feature = "plus")]
            plan: plan.map(Into::into),
            #[cfg(not(feature = "plus"))]
            plan: None,
            invite: invite.map(Into::into),
        }
    }
}

#[async_trait]
impl SubCmd for Signup {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonEmpty = self
            .backend
            .send_with(
                |client| async move { client.auth_signup_post().body(self.clone()).send().await },
                true,
            )
            .await?;
        Ok(())
    }
}
