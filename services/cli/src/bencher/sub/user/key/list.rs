use bencher_client::types::{JsonDirection, UserKeysSort};
use bencher_json::{ResourceName, UserResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::{
        CliPagination,
        user::key::{CliUserKeyList, CliUserKeysSort},
    },
};

#[derive(Debug)]
pub struct List {
    pub user: UserResourceId,
    pub name: Option<ResourceName>,
    pub search: Option<String>,
    pub revoked: bool,
    pub pagination: Pagination,
    pub backend: AuthBackend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<UserKeysSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliUserKeyList> for List {
    type Error = CliError;

    fn try_from(list: CliUserKeyList) -> Result<Self, Self::Error> {
        let CliUserKeyList {
            user,
            name,
            search,
            revoked,
            pagination,
            backend,
        } = list;
        Ok(Self {
            user,
            name,
            search,
            revoked,
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliUserKeysSort>> for Pagination {
    fn from(pagination: CliPagination<CliUserKeysSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliUserKeysSort::Name => UserKeysSort::Name,
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
                let mut client = client.user_keys_get().user(self.user.clone());
                if let Some(name) = self.name.clone() {
                    client = client.name(name);
                }
                if let Some(search) = self.search.clone() {
                    client = client.search(search);
                }
                if self.revoked {
                    client = client.revoked(true);
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
