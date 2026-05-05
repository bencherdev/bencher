use bencher_client::types::{JsonDirection, ProjKeysSort};
use bencher_json::{ProjectResourceId, ResourceName};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::{
        CliPagination,
        project::key::{CliProjectKeyList, CliProjectKeysSort},
    },
};

#[derive(Debug, Clone)]
pub struct List {
    pub project: ProjectResourceId,
    pub name: Option<ResourceName>,
    pub search: Option<String>,
    pub revoked: bool,
    pub pagination: Pagination,
    pub backend: AuthBackend,
}

#[derive(Debug, Clone)]
pub struct Pagination {
    pub sort: Option<ProjKeysSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliProjectKeyList> for List {
    type Error = CliError;

    fn try_from(list: CliProjectKeyList) -> Result<Self, Self::Error> {
        let CliProjectKeyList {
            project,
            name,
            search,
            revoked,
            pagination,
            backend,
        } = list;
        Ok(Self {
            project,
            name,
            search,
            revoked,
            pagination: pagination.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliProjectKeysSort>> for Pagination {
    fn from(pagination: CliPagination<CliProjectKeysSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliProjectKeysSort::Name => ProjKeysSort::Name,
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
                let mut request = client.proj_keys_get().project(self.project.clone());
                if let Some(name) = self.name.clone() {
                    request = request.name(name);
                }
                if let Some(search) = self.search.clone() {
                    request = request.search(search);
                }
                if self.revoked {
                    request = request.revoked(true);
                }
                if let Some(sort) = self.pagination.sort {
                    request = request.sort(sort);
                }
                if let Some(direction) = self.pagination.direction {
                    request = request.direction(direction);
                }
                if let Some(per_page) = self.pagination.per_page {
                    request = request.per_page(per_page);
                }
                if let Some(page) = self.pagination.page {
                    request = request.page(page);
                }
                request.send().await
            })
            .await?;
        Ok(())
    }
}
