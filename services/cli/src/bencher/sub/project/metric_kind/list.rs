use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonDirection, ProjMetricKindsSort};
use bencher_json::{JsonMetricKinds, NonEmpty, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::{
        project::metric_kind::{CliMetricKindList, CliMetricKindsSort},
        CliPagination,
    },
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub project: ResourceId,
    pub name: Option<NonEmpty>,
    pub pagination: Pagination,
    pub backend: Backend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<ProjMetricKindsSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliMetricKindList> for List {
    type Error = CliError;

    fn try_from(list: CliMetricKindList) -> Result<Self, Self::Error> {
        let CliMetricKindList {
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

impl From<CliPagination<CliMetricKindsSort>> for Pagination {
    fn from(pagination: CliPagination<CliMetricKindsSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliMetricKindsSort::Name => ProjMetricKindsSort::Name,
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
        let _: JsonMetricKinds = self
            .backend
            .send_with(
                |client| async move {
                    let mut client = client.proj_metric_kinds_get().project(self.project.clone());
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
                },
                true,
            )
            .await?;
        Ok(())
    }
}
