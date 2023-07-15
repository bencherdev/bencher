use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonEmpty, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::CliProjectDelete,
    CliError,
};

#[derive(Debug)]
pub struct Delete {
    pub org: Option<ResourceId>,
    pub project: ResourceId,
    pub backend: Backend,
}

impl TryFrom<CliProjectDelete> for Delete {
    type Error = CliError;

    fn try_from(view: CliProjectDelete) -> Result<Self, Self::Error> {
        let CliProjectDelete {
            org,
            project,
            backend,
        } = view;
        Ok(Self {
            org,
            project,
            backend: backend.try_into()?,
        })
    }
}

#[async_trait]
impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonEmpty = self
            .backend
            .send_with(
                |client| async move {
                    if let Some(org) = self.org.clone() {
                        client
                            .org_project_delete()
                            .organization(org)
                            .project(self.project.clone())
                            .send()
                            .await
                    } else {
                        client
                            .project_delete()
                            .project(self.project.clone())
                            .send()
                            .await
                    }
                },
                true,
            )
            .await?;
        Ok(())
    }
}
