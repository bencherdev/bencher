use bencher_client::types::{JsonDirection, UsersSort};
use bencher_json::UserName;

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::{
        user::{CliUserList, CliUsersSort},
        CliPagination,
    },
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub name: Option<UserName>,
    pub search: Option<String>,
    pub pagination: Pagination,
    pub backend: AuthBackend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<UsersSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliUserList> for List {
    type Error = CliError;

    fn try_from(list: CliUserList) -> Result<Self, Self::Error> {
        let CliUserList {
            name,
            search,
            pagination,
            backend,
        } = list;
        Ok(Self {
            name,
            search,
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliUsersSort>> for Pagination {
    fn from(pagination: CliPagination<CliUsersSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliUsersSort::Name => UsersSort::Name,
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
                let mut client = client.users_get();
                if let Some(name) = self.name.clone() {
                    client = client.name(name);
                }
                if let Some(search) = self.search.clone() {
                    client = client.search(search);
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
