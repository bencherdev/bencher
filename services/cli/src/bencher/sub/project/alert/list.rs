use bencher_client::types::{AlertStatus, JsonDirection, ProjAlertsSort};
use bencher_json::ResourceId;

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::{
        project::alert::{CliAlertList, CliAlertsSort},
        CliPagination,
    },
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub project: ResourceId,
    pub pagination: Pagination,
    pub status: Option<AlertStatus>,
    pub archived: bool,
    pub backend: PubBackend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<ProjAlertsSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliAlertList> for List {
    type Error = CliError;

    fn try_from(list: CliAlertList) -> Result<Self, Self::Error> {
        let CliAlertList {
            project,
            pagination,
            status,
            archived,
            backend,
        } = list;
        Ok(Self {
            project,
            pagination: pagination.into(),
            status: status.map(Into::into),
            archived,
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliAlertsSort>> for Pagination {
    fn from(pagination: CliPagination<CliAlertsSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliAlertsSort::Created => ProjAlertsSort::Created,
                CliAlertsSort::Modified => ProjAlertsSort::Modified,
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
                let mut client = client.proj_alerts_get().project(self.project.clone());
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
                if let Some(status) = self.status {
                    client = client.status(status);
                }
                if self.archived {
                    client = client.archived(self.archived);
                }
                client.send().await
            })
            .await?;
        Ok(())
    }
}
