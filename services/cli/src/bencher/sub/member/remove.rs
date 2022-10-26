use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::member::CliMemberRemove,
    CliError,
};

#[derive(Debug)]
pub struct Remove {
    pub org: ResourceId,
    pub user: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliMemberRemove> for Remove {
    type Error = CliError;

    fn try_from(remove: CliMemberRemove) -> Result<Self, Self::Error> {
        let CliMemberRemove { org, user, backend } = remove;
        Ok(Self {
            org,
            user,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Remove {
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
        self.backend
            .delete(&format!(
                "/v0/organizations/{}/members/{}",
                self.org, self.user
            ))
            .await?;
        Ok(())
    }
}
