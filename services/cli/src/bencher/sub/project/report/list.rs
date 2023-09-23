use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonDirection, ProjReportsSort};
use bencher_json::{JsonReports, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::{
        project::report::{CliReportList, CliReportsSort},
        CliPagination,
    },
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub project: ResourceId,
    pub pagination: Pagination,
    pub backend: Backend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<ProjReportsSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliReportList> for List {
    type Error = CliError;

    fn try_from(list: CliReportList) -> Result<Self, Self::Error> {
        let CliReportList {
            project,
            pagination,
            backend,
        } = list;
        Ok(Self {
            project,
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliReportsSort>> for Pagination {
    fn from(pagination: CliPagination<CliReportsSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliReportsSort::DateTime => ProjReportsSort::DateTime,
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
        let _json: JsonReports = self
            .backend
            .send_with(
                |client| async move {
                    let mut client = client.proj_reports_get().project(self.project.clone());
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
