use bencher_client::types::{JsonDirection, RunnersSort};
use bencher_json::ResourceName;

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::{
        CliPagination,
        system::runner::{CliRunnerList, CliRunnersSort},
    },
};

#[derive(Debug)]
pub struct List {
    pub name: Option<ResourceName>,
    pub search: Option<String>,
    pub archived: bool,
    pub pagination: Pagination,
    pub backend: AuthBackend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<RunnersSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliRunnerList> for List {
    type Error = CliError;

    fn try_from(list: CliRunnerList) -> Result<Self, Self::Error> {
        let CliRunnerList {
            name,
            search,
            archived,
            pagination,
            backend,
        } = list;
        Ok(Self {
            name,
            search,
            archived,
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliRunnersSort>> for Pagination {
    fn from(pagination: CliPagination<CliRunnersSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliRunnersSort::Name => RunnersSort::Name,
                CliRunnersSort::Created => RunnersSort::Created,
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
                let mut client = client.runners_get();
                if let Some(name) = self.name.clone() {
                    client = client.name(name);
                }
                if let Some(search) = self.search.clone() {
                    client = client.search(search);
                }
                if self.archived {
                    client = client.archived(self.archived);
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
