use bencher_client::types::JsonSignup;
#[cfg(feature = "plus")]
use bencher_client::types::PlanLevel;
use bencher_json::{Email, Jwt, Slug, UserName};

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::system::auth::CliAuthSignup,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Signup {
    pub name: UserName,
    pub slug: Option<Slug>,
    pub email: Email,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
    pub invite: Option<Jwt>,
    pub i_agree: bool,
    pub backend: PubBackend,
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
            i_agree,
            backend,
        } = signup;
        Ok(Self {
            name,
            slug,
            email,
            #[cfg(feature = "plus")]
            plan: plan.map(Into::into),
            invite,
            i_agree,
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
            i_agree,
            ..
        } = signup;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
            email: email.into(),
            #[cfg(feature = "plus")]
            plan,
            #[cfg(not(feature = "plus"))]
            plan: None,
            invite: invite.map(Into::into),
            i_agree,
        }
    }
}

impl SubCmd for Signup {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.auth_signup_post().body(self.clone()).send().await })
            .await?;
        Ok(())
    }
}
