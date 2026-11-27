use bencher_client::types::{ChangepointStatus, JsonDirection, ProjChangepointsSort};
use bencher_json::{
    BenchmarkNameId, BranchNameId, MeasureNameId, ProjectResourceId, TestbedNameId,
    project::changepoint::JsonChangepointQuery,
};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::{
        CliPagination,
        project::changepoint::{CliChangepointList, CliChangepointStatus, CliChangepointsSort},
    },
};

#[derive(Debug, Clone)]
pub struct List {
    pub project: ProjectResourceId,
    pub pagination: Pagination,
    pub status: Option<ChangepointStatus>,
    pub branch: Option<BranchNameId>,
    pub testbed: Option<TestbedNameId>,
    pub benchmark: Option<BenchmarkNameId>,
    pub measure: Option<MeasureNameId>,
    pub archived: bool,
    pub backend: PubBackend,
}

#[derive(Debug, Clone)]
pub struct Pagination {
    pub sort: Option<ProjChangepointsSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliChangepointList> for List {
    type Error = CliError;

    fn try_from(list: CliChangepointList) -> Result<Self, Self::Error> {
        let CliChangepointList {
            project,
            pagination,
            status,
            branch,
            testbed,
            benchmark,
            measure,
            archived,
            backend,
        } = list;
        Ok(Self {
            project,
            pagination: pagination.into(),
            status: status.map(Into::into),
            branch,
            testbed,
            benchmark,
            measure,
            archived,
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliChangepointsSort>> for Pagination {
    fn from(pagination: CliPagination<CliChangepointsSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliChangepointsSort::Created => ProjChangepointsSort::Created,
                CliChangepointsSort::Modified => ProjChangepointsSort::Modified,
            }),
            direction: direction.map(Into::into),
            page,
            per_page,
        }
    }
}

impl From<CliChangepointStatus> for ChangepointStatus {
    fn from(status: CliChangepointStatus) -> Self {
        match status {
            CliChangepointStatus::Active => Self::Active,
            CliChangepointStatus::Dismissed => Self::Dismissed,
            CliChangepointStatus::Silenced => Self::Silenced,
            CliChangepointStatus::Withdrawn => Self::Withdrawn,
        }
    }
}

impl From<List> for JsonChangepointQuery {
    fn from(list: List) -> Self {
        let List {
            branch,
            testbed,
            benchmark,
            measure,
            archived,
            ..
        } = list;
        Self {
            status: None,
            branch,
            testbed,
            benchmark,
            measure,
            archived: archived.then_some(archived),
        }
    }
}

impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        let query: &JsonChangepointQuery = &self.clone().into();
        let _json = self
            .backend
            .send(|client| async move {
                let mut client = client.proj_changepoints_get().project(self.project.clone());
                if let Some(status) = self.status {
                    client = client.status(status);
                }
                if let Some(branch) = query.branch() {
                    client = client.branch(branch);
                }
                if let Some(testbed) = query.testbed() {
                    client = client.testbed(testbed);
                }
                if let Some(benchmark) = query.benchmark() {
                    client = client.benchmark(benchmark);
                }
                if let Some(measure) = query.measure() {
                    client = client.measure(measure);
                }
                if let Some(archived) = query.archived {
                    client = client.archived(archived);
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
