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
        if let Some(org) = &self.org {
            self.backend
                .get(&format!("/v0/organizations/{org}/projects"))
                .await?;
        } else {
            let json_projects: JsonProjects = self.clone().into();
            self.backend
                .get_query("/v0/projects", &json_projects)
                .await?;
        }

        Ok(())
    }
}
