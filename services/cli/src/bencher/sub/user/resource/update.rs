use bencher_client::types::JsonUpdateUser;
use bencher_json::{Email, ResourceId, Slug, UserName};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::user::CliUserUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub user: ResourceId,
    pub name: Option<UserName>,
    pub slug: Option<Slug>,
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
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .user_patch()
                    .user(self.user.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
