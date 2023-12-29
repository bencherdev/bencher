use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonDirection, OrgMembersSort};
use bencher_json::{ResourceId, UserName};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::{
        organization::member::{CliMemberList, CliMembersSort},
        CliPagination,
    },
    CliError,
};

#[derive(Debug)]
pub struct List {
    pub org: ResourceId,
    pub name: Option<UserName>,
    pub pagination: Pagination,
    pub backend: AuthBackend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<OrgMembersSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliMemberList> for List {
    type Error = CliError;

    fn try_from(list: CliMemberList) -> Result<Self, Self::Error> {
        let CliMemberList {
            org,
            name,
            pagination,
            backend,
        } = list;
        Ok(Self {
            org,
            name,
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliMembersSort>> for Pagination {
    fn from(pagination: CliPagination<CliMembersSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliMembersSort::Name => OrgMembersSort::Name,
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
        let _json = self
            .backend
            .as_ref()
            .send(|client| async move {
                let mut client = client.org_members_get().organization(self.org.clone());
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
            })
            .await?;
        Ok(())
    }
}
