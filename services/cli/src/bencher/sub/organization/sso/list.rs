use bencher_client::types::{JsonDirection, OrgSsoSort};
use bencher_json::OrganizationResourceId;

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::{
        CliPagination,
        organization::sso::{CliSsoList, CliSsoSort},
    },
};

#[derive(Debug)]
pub struct List {
    pub organization: OrganizationResourceId,
    pub pagination: Pagination,
    pub backend: PubBackend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<OrgSsoSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliSsoList> for List {
    type Error = CliError;

    fn try_from(list: CliSsoList) -> Result<Self, Self::Error> {
        let CliSsoList {
            organization,
            pagination,
            backend,
        } = list;
        Ok(Self {
            organization,
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliSsoSort>> for Pagination {
    fn from(pagination: CliPagination<CliSsoSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliSsoSort::Domain => OrgSsoSort::Domain,
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
                    .org_ssos_get()
                    .organization(self.organization.clone());
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
