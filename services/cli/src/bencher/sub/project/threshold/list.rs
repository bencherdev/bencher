use bencher_client::types::{JsonDirection, ProjThresholdsSort};
use bencher_json::{
    BranchNameId, MeasureNameId, ResourceId, TestbedNameId, project::threshold::JsonThresholdQuery,
};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::{
        CliPagination,
        project::threshold::{CliThresholdList, CliThresholdsSort},
    },
};

#[derive(Debug, Clone)]
pub struct List {
    pub project: ResourceId,
    pub branch: Option<BranchNameId>,
    pub testbed: Option<TestbedNameId>,
    pub measure: Option<MeasureNameId>,
    pub pagination: Pagination,
    pub archived: bool,
    pub backend: PubBackend,
}

#[derive(Debug, Clone)]
pub struct Pagination {
    pub sort: Option<ProjThresholdsSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliThresholdList> for List {
    type Error = CliError;

    fn try_from(list: CliThresholdList) -> Result<Self, Self::Error> {
        let CliThresholdList {
            project,
            branch,
            testbed,
            measure,
            pagination,
            archived,
            backend,
        } = list;
        Ok(Self {
            project,
            branch,
            testbed,
            measure,
            pagination: pagination.into(),
            archived,
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliThresholdsSort>> for Pagination {
    fn from(pagination: CliPagination<CliThresholdsSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliThresholdsSort::Created => ProjThresholdsSort::Created,
                CliThresholdsSort::Modified => ProjThresholdsSort::Modified,
            }),
            direction: direction.map(Into::into),
            page,
            per_page,
        }
    }
}

impl From<List> for JsonThresholdQuery {
    fn from(list: List) -> Self {
        let List {
            branch,
            testbed,
            measure,
            archived,
            ..
        } = list;
        Self {
            branch,
            testbed,
            measure,
            archived: archived.then_some(archived),
        }
    }
}

impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        let json_threshold_query: &JsonThresholdQuery = &self.clone().into();
        let _json = self
            .backend
            .send(|client| async move {
                let mut client = client.proj_thresholds_get().project(self.project.clone());

                if let Some(branch) = json_threshold_query.branch() {
                    client = client.branch(branch);
                }
                if let Some(testbed) = json_threshold_query.testbed() {
                    client = client.testbed(testbed);
                }
                if let Some(measure) = json_threshold_query.measure() {
                    client = client.measure(measure);
                }

                if let Some(archived) = json_threshold_query.archived {
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
