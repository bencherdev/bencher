use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::organization::member::CliMemberView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub org: ResourceId,
    pub user: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliMemberView> for View {
    type Error = CliError;

    fn try_from(view: CliMemberView) -> Result<Self, Self::Error> {
        let CliMemberView { org, user, backend } = view;
        Ok(Self {
            org,
            user,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for View {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend
            .get(&format!(
                "/v0/organizations/{}/members/{}",
                self.org, self.user
            ))
            .await?;
        Ok(())
    }
}
