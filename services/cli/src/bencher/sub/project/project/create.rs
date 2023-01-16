use std::{convert::TryFrom, str::FromStr};

use async_trait::async_trait;
use bencher_json::{JsonNewProject, NonEmpty, ResourceId, Slug, Url};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::CliProjectCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub org: ResourceId,
    pub name: NonEmpty,
    pub slug: Option<Slug>,
    pub url: Option<Url>,
    pub public: Option<bool>,
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
            public,
            private,
            backend,
        } = create;
        Ok(Self {
            org,
            name: NonEmpty::from_str(&name)?,
            slug: if let Some(slug) = slug {
                Some(Slug::from_str(&slug)?)
            } else {
                None
            },
            url: map_url(url)?,
            public: Some(if public { true } else { !private }),
            backend: backend.try_into()?,
        })
    }
}

pub fn map_url(url: Option<String>) -> Result<Option<Url>, CliError> {
    Ok(if let Some(url) = url {
        Some(Url::from_str(&url)?)
    } else {
        None
    })
}

impl From<Create> for JsonNewProject {
    fn from(create: Create) -> Self {
        let Create {
            name,
            slug,
            url,
            public,
            ..
        } = create;
        Self {
            name,
            slug,
            url,
            public,
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let project: JsonNewProject = self.clone().into();
        self.backend
            .post(
                &format!("/v0/organizations/{}/projects", self.org),
                &project,
            )
            .await?;
        Ok(())
    }
}
