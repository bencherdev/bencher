use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonDirection, ProjReportsSort};
use bencher_json::{project::report::JsonReportQuery, DateTime, NameId, ResourceId};

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::{
        project::report::{CliReportList, CliReportsSort},
        CliPagination,
    },
    CliError,
};

#[derive(Debug, Clone)]
pub struct List {
    pub project: ResourceId,
    pub branch: Option<NameId>,
    pub testbed: Option<NameId>,
    pub start_time: Option<DateTime>,
    pub end_time: Option<DateTime>,
    pub pagination: Pagination,
    pub backend: PubBackend,
}

#[derive(Debug, Clone)]
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
            branch,
            testbed,
            start_time,
            end_time,
            pagination,
            backend,
        } = list;
        Ok(Self {
            project,
            branch,
            testbed,
            start_time,
            end_time,
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

impl From<List> for JsonReportQuery {
    fn from(list: List) -> Self {
        let List {
            branch,
            testbed,
            start_time,
            end_time,
            ..
        } = list;
        Self {
            branch,
            testbed,
            start_time,
            end_time,
        }
    }
}

#[async_trait]
impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        let json_report_query: &JsonReportQuery = &self.clone().into();
        let _json = self
            .backend
            .send(|client| async move {
                let mut client = client.proj_reports_get().project(self.project.clone());

                if let Some(branch) = json_report_query.branch() {
                    client = client.branch(branch);
                }
                if let Some(testbed) = json_report_query.testbed() {
                    client = client.testbed(testbed);
                }

                if let Some(start_time) = json_report_query.start_time() {
                    client = client.start_time(start_time);
                }
                if let Some(end_time) = json_report_query.end_time() {
                    client = client.end_time(end_time);
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
