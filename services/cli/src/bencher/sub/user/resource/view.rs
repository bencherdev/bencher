use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::user::CliUserView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub user: ResourceId,
    pub backend: Backend,
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

#[async_trait]
impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move { client.user_get().user(self.user.clone()).send().await })
            .await?;
        Ok(())
    }
}
