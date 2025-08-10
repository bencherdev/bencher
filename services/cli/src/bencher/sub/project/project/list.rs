use bencher_client::types::{JsonDirection, OrgProjectsSort, ProjectsSort};
use bencher_json::{OrganizationResourceId, ResourceName};

use crate::{
    CliError,
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::{
        CliPagination,
        project::{CliProjectList, CliProjectsSort},
    },
};

#[derive(Debug)]
pub struct List {
    pub organization: Option<OrganizationResourceId>,
    pub name: Option<ResourceName>,
    pub search: Option<String>,
    pub pagination: Pagination,
    pub backend: PubBackend,
}

#[derive(Debug)]
pub struct Pagination {
    pub org_projects_sort: Option<OrgProjectsSort>,
    pub projects_sort: Option<ProjectsSort>,
    pub direction: Option<JsonDirection>,
    pub per_page: Option<u8>,
    pub page: Option<u32>,
}

impl TryFrom<CliProjectList> for List {
    type Error = CliError;

    fn try_from(list: CliProjectList) -> Result<Self, Self::Error> {
        let CliProjectList {
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

impl From<CliPagination<CliProjectsSort>> for Pagination {
    fn from(pagination: CliPagination<CliProjectsSort>) -> Self {
        let CliPagination {
            sort,
            direction,
            per_page,
            page,
        } = pagination;
        Self {
            org_projects_sort: sort.map(|sort| match sort {
                CliProjectsSort::Name => OrgProjectsSort::Name,
            }),
            projects_sort: sort.map(|sort| match sort {
                CliProjectsSort::Name => ProjectsSort::Name,
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
                if let Some(organization) = self.organization.clone() {
                    let mut client = client.org_projects_get().organization(organization);
                    if let Some(name) = self.name.clone() {
                        client = client.name(name);
                    }
                    if let Some(search) = self.search.clone() {
                        client = client.search(search);
                    }
                    if let Some(sort) = self.pagination.org_projects_sort {
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
                } else {
                    let mut client = client.projects_get();
                    if let Some(name) = self.name.clone() {
                        client = client.name(name);
                    }
                    if let Some(search) = self.search.clone() {
                        client = client.search(search);
                    }
                    if let Some(sort) = self.pagination.projects_sort {
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
                }
            })
            .await?;
        Ok(())
    }
}
