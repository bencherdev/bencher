use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonDirection, ProjBenchmarksSort};
use bencher_json::{BenchmarkName, ResourceId};

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::{
        project::benchmark::{CliBenchmarkList, CliBenchmarksSort},
        CliPagination,
    },
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub project: ResourceId,
    pub name: Option<BenchmarkName>,
    pub pagination: Pagination,
    pub backend: PubBackend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<ProjBenchmarksSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliBenchmarkList> for List {
    type Error = CliError;

    fn try_from(list: CliBenchmarkList) -> Result<Self, Self::Error> {
        let CliBenchmarkList {
            project,
            name,
            pagination,
            backend,
        } = list;
        Ok(Self {
            project,
            name,
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliBenchmarksSort>> for Pagination {
    fn from(pagination: CliPagination<CliBenchmarksSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliBenchmarksSort::Name => ProjBenchmarksSort::Name,
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
        let _json = self
            .backend
            .as_ref()
            .send(|client| async move {
                let mut client = client.proj_benchmarks_get().project(self.project.clone());
                if let Some(name) = self.name.clone() {
                    client = client.name(name);
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
