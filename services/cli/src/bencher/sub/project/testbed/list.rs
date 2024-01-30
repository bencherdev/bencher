use std::convert::TryFrom;

use bencher_client::types::{JsonDirection, ProjTestbedsSort};
use bencher_json::{ResourceId, ResourceName};

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::{
        project::testbed::{CliTestbedList, CliTestbedsSort},
        CliPagination,
    },
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub project: ResourceId,
    pub name: Option<ResourceName>,
    pub search: Option<String>,
    pub pagination: Pagination,
    pub backend: PubBackend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<ProjTestbedsSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliTestbedList> for List {
    type Error = CliError;

    fn try_from(list: CliTestbedList) -> Result<Self, Self::Error> {
        let CliTestbedList {
            project,
            name,
            search,
            pagination,
            backend,
        } = list;
        Ok(Self {
            project,
            name,
            search,
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliTestbedsSort>> for Pagination {
    fn from(pagination: CliPagination<CliTestbedsSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliTestbedsSort::Name => ProjTestbedsSort::Name,
            }),
            direction: direction.map(Into::into),
            page,
            per_page,
        }
    }
}

impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                let mut client = client.proj_testbeds_get().project(self.project.clone());
                if let Some(name) = self.name.clone() {
                    client = client.name(name);
                }
                if let Some(search) = self.search.clone() {
                    client = client.search(search);
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
            })
            .await?;
        Ok(())
    }
}
