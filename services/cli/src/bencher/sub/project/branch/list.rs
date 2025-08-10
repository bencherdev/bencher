use bencher_client::types::{JsonDirection, ProjBranchesSort};
use bencher_json::{BranchName, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::{
        CliPagination,
        project::branch::{CliBranchList, CliBranchesSort},
    },
};

#[derive(Debug)]
pub struct List {
    pub project: ProjectResourceId,
    pub name: Option<BranchName>,
    pub search: Option<String>,
    pub pagination: Pagination,
    pub archived: bool,
    pub backend: PubBackend,
}

#[derive(Debug)]
pub struct Pagination {
    pub sort: Option<ProjBranchesSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliBranchList> for List {
    type Error = CliError;

    fn try_from(list: CliBranchList) -> Result<Self, Self::Error> {
        let CliBranchList {
            project,
            name,
            search,
            pagination,
            archived,
            backend,
        } = list;
        Ok(Self {
            project,
            name,
            search,
            pagination: pagination.into(),
            archived,
            backend: backend.try_into()?,
        })
    }
}

impl From<CliPagination<CliBranchesSort>> for Pagination {
    fn from(pagination: CliPagination<CliBranchesSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            sort: sort.map(|sort| match sort {
                CliBranchesSort::Name => ProjBranchesSort::Name,
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
                let mut client = client.proj_branches_get().project(self.project.clone());
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
                if self.archived {
                    client = client.archived(self.archived);
                }
                client.send().await
            })
            .await?;
        Ok(())
    }
}
