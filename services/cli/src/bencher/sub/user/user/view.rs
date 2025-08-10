use bencher_json::UserResourceId;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::user::CliUserView,
};

#[derive(Debug)]
pub struct View {
    pub user: UserResourceId,
    pub backend: AuthBackend,
}

impl TryFrom<CliUserView> for View {
    type Error = CliError;

    fn try_from(view: CliUserView) -> Result<Self, Self::Error> {
        let CliUserView { user, backend } = view;
        Ok(Self {
            user,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.user_get().user(self.user.clone()).send().await })
            .await?;
        Ok(())
    }
}
