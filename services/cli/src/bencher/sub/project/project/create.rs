use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonNewProject, Visibility};
use bencher_json::{JsonProject, NonEmpty, ResourceId, Slug, Url};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::{CliProjectCreate, CliProjectVisibility},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub org: ResourceId,
    pub name: NonEmpty,
    pub slug: Option<Slug>,
    pub url: Option<Url>,
    pub visibility: Option<Visibility>,
    pub backend: Backend,
}

impl TryFrom<CliProjectCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliProjectCreate) -> Result<Self, Self::Error> {
        let CliProjectCreate {
            org,
            name,
            slug,
            url,
            visibility,
            backend,
        } = create;
        Ok(Self {
            org,
            name,
            slug,
            url,
            visibility: visibility.map(Into::into),
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
            visibility,
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonProject = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .org_project_post()
                        .organization(self.org.clone())
                        .body(self.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
