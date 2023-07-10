use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonDirection, ProjectsSort};
use bencher_json::{NonEmpty, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::{
        project::{CliProjectList, CliProjectsSort},
        CliPagination,
    },
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub org: Option<ResourceId>,
    pub public: Option<bool>,
    pub name: Option<NonEmpty>,
    pub pagination: Pagination,
    pub backend: Backend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<ProjectsSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliProjectList> for List {
    type Error = CliError;

    fn try_from(list: CliProjectList) -> Result<Self, Self::Error> {
        let CliProjectList {
            org,
            public,
            name,
            pagination,
            backend,
        } = list;
        Ok(Self {
            org,
            public: if public { Some(public) } else { None },
            name,
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliProjectsSort>> for Pagination {
    fn from(pagination: CliPagination<CliProjectsSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliProjectsSort::Name => ProjectsSort::Name,
            }),
            direction: direction.map(Into::into),
            page,
            per_page,
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
                        let mut client = client.projects_get();
                        if let Some(public) = self.public {
                            client = client.public(public);
                        }
                        if let Some(name) = &self.name {
                            client = client.name(name.as_ref());
                        }
                        if let Some(sort) = self.pagination.sort {
                            client = client.sort(sort);
                        }
                        if let Some(direction) = self.pagination.direction {
                            client = client.direction(direction);
                        }
                        if let Some(per_page) = self.pagination.per_page {
                            client = client.per_page(per_page);
                        }
                        if let Some(page) = self.pagination.page {
                            client = client.page(page);
                        }
                        client.send().await
                    }
                },
                true,
            )
            .await?;
        Ok(())
    }
}
