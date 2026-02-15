use bencher_client::types::{JobStatus, JsonDirection, ProjJobsSort};
use bencher_json::ProjectResourceId;

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::{
        CliPagination,
        project::job::{CliJobList, CliJobStatus, CliJobsSort},
    },
};

#[derive(Debug)]
pub struct List {
    pub project: ProjectResourceId,
    pub status: Option<JobStatus>,
    pub pagination: Pagination,
    pub backend: PubBackend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<ProjJobsSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliJobList> for List {
    type Error = CliError;

    fn try_from(list: CliJobList) -> Result<Self, Self::Error> {
        let CliJobList {
            project,
            status,
            pagination,
            backend,
        } = list;
        Ok(Self {
            project,
            status: status.map(Into::into),
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliJobsSort>> for Pagination {
    fn from(pagination: CliPagination<CliJobsSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliJobsSort::Created => ProjJobsSort::Created,
            }),
            direction: direction.map(Into::into),
            page,
            per_page,
        }
    }
}

impl From<CliJobStatus> for JobStatus {
    fn from(status: CliJobStatus) -> Self {
        match status {
            CliJobStatus::Pending => Self::Pending,
            CliJobStatus::Claimed => Self::Claimed,
            CliJobStatus::Running => Self::Running,
            CliJobStatus::Completed => Self::Completed,
            CliJobStatus::Failed => Self::Failed,
            CliJobStatus::Canceled => Self::Canceled,
        }
    }
}

impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                let mut client = client.proj_jobs_get().project(self.project.clone());

                if let Some(status) = self.status {
                    client = client.status(status);
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
