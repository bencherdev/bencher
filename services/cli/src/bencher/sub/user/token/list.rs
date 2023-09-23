use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonDirection, UserTokensSort};
use bencher_json::{JsonTokens, NonEmpty, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::{
        user::token::{CliTokenList, CliTokensSort},
        CliPagination,
    },
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub user: ResourceId,
    pub name: Option<NonEmpty>,
    pub pagination: Pagination,
    pub backend: Backend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<UserTokensSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliTokenList> for List {
    type Error = CliError;

    fn try_from(list: CliTokenList) -> Result<Self, Self::Error> {
        let CliTokenList {
            user,
            name,
            pagination,
            backend,
        } = list;
        Ok(Self {
            user,
            name,
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliTokensSort>> for Pagination {
    fn from(pagination: CliPagination<CliTokensSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliTokensSort::Name => UserTokensSort::Name,
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
        let _json: JsonTokens = self
            .backend
            .send_with(
                |client| async move {
                    let mut client = client.user_tokens_get().user(self.user.clone());
                    if let Some(name) = self.name.clone() {
                        client = client.name(name);
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
                },
                true,
            )
            .await?;
        Ok(())
    }
}
