use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonMember, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::organization::member::CliMemberRemove,
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
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonMember = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .org_member_delete()
                        .organization(self.org.clone())
                        .user(self.user.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
