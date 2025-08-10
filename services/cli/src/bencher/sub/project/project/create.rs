use bencher_client::types::{JsonNewProject, Visibility};
use bencher_json::{OrganizationResourceId, ProjectSlug, ResourceName, Url};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::{CliProjectCreate, CliProjectVisibility},
};

#[derive(Debug, Clone)]
pub struct Create {
    pub organization: OrganizationResourceId,
    pub name: ResourceName,
    pub slug: Option<ProjectSlug>,
    pub url: Option<Url>,
    pub visibility: Visibility,
    pub backend: AuthBackend,
}

impl TryFrom<CliProjectCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliProjectCreate) -> Result<Self, Self::Error> {
        let CliProjectCreate {
            organization,
            name,
            slug,
            url,
            visibility,
            backend,
        } = create;
        Ok(Self {
            organization,
            name,
            slug,
            url,
            visibility: visibility.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliProjectVisibility> for Visibility {
    fn from(visibility: CliProjectVisibility) -> Self {
        match visibility {
            CliProjectVisibility::Public => Self::Public,
            #[cfg(feature = "plus")]
            CliProjectVisibility::Private => Self::Private,
        }
    }
}

impl From<Create> for JsonNewProject {
    fn from(create: Create) -> Self {
        let Create {
            name,
            slug,
            url,
            visibility,
            ..
        } = create;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
            url: url.map(Into::into),
            visibility: Some(visibility),
        }
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_project_post()
                    .organization(self.organization.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
