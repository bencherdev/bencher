use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonDirection, ProjThresholdsSort};
use bencher_json::{JsonThresholds, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::{
        project::threshold::{CliThresholdList, CliThresholdsSort},
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

#[async_trait]
impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonThresholds = self
            .backend
            .send_with(|client| async move {
                let mut client = client.proj_thresholds_get().project(self.project.clone());
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
