use bencher_client::types::JsonUpdateUser;
use bencher_json::{Email, JsonUser, UserName, UserResourceId, UserSlug};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    cli_eprintln,
    parser::user::CliUserUpdate,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub user: UserResourceId,
    pub name: Option<UserName>,
    pub slug: Option<UserSlug>,
    pub email: Option<Email>,
    pub admin: Option<bool>,
    pub locked: Option<bool>,
    pub backend: AuthBackend,
}

impl TryFrom<CliUserUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliUserUpdate) -> Result<Self, Self::Error> {
        let CliUserUpdate {
            user,
            name,
            slug,
            email,
            admin,
            locked,
            backend,
        } = create;
        Ok(Self {
            user,
            name,
            slug,
            email,
            admin,
            locked,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateUser {
    fn from(update: Update) -> Self {
        let Update {
            name,
            slug,
            email,
            admin,
            locked,
            ..
        } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            email: email.map(Into::into),
            admin,
            locked,
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let json: JsonUser = self
            .backend
            .send_with(|client| async move {
                client
                    .user_patch()
                    .user(self.user.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        if let Some(email) = &self.email
            && *email != json.email
        {
            cli_eprintln!("A confirm link was sent to {email}.");
        }
        Ok(())
    }
}
