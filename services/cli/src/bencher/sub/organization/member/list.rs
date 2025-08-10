use bencher_client::types::{JsonDirection, OrgMembersSort};
use bencher_json::{OrganizationResourceId, UserName};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::{
        CliPagination,
        organization::member::{CliMemberList, CliMembersSort},
    },
};

#[derive(Debug)]
pub struct List {
    pub organization: OrganizationResourceId,
    pub name: Option<UserName>,
    pub search: Option<String>,
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
            organization,
            name,
            search,
            pagination,
            backend,
        } = list;
        Ok(Self {
            organization,
            name,
            search,
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

impl SubCmd for List {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                let mut client = client
                    .org_members_get()
                    .organization(self.organization.clone());
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
