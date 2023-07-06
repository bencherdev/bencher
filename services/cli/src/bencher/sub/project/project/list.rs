use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{project::JsonProjects, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::CliProjectList,
    CliError,
};

#[derive(Debug, Clone)]
pub struct List {
    pub org: Option<ResourceId>,
    pub public: bool,
    pub backend: Backend,
}

impl TryFrom<CliProjectList> for List {
    type Error = CliError;

    fn try_from(list: CliProjectList) -> Result<Self, Self::Error> {
        let CliProjectList {
            org,
            public,
            backend,
        } = list;
        Ok(Self {
            org,
            public,
            backend: backend.try_into()?,
        })
    }
}

impl From<List> for JsonProjects {
    fn from(list: List) -> Self {
        let List { public, .. } = list;
        Self {
            public: Some(public),
        }
    }
}

#[async_trait]
impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        self.backend
            .send_with(
                |client| async move {
                    if let Some(org) = self.org.clone() {
                        client.org_projects_get().organization(org).send().await
                    } else {
                        client.projects_get().public(self.public).send().await
                    }
                },
                true,
            )
            .await?;
        Ok(())
    }
}
