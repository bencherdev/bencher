use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::CliProjectList,
    CliError,
};

#[derive(Debug)]
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

#[async_trait]
impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        if let Some(org) = &self.org {
            self.backend
                .get(&format!("/v0/organizations/{org}/projects"))
                .await?;
        } else {
            self.backend
                .get_query(
                    "/v0/projects",
                    vec![("public".into(), self.public.to_string())],
                )
                .await?;
        }

        Ok(())
    }
}
