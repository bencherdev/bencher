use std::{convert::TryFrom, str::FromStr};

use async_trait::async_trait;
use bencher_json::{JsonNewProject, ResourceId, Slug};
use url::Url;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::CliProjectCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub org: ResourceId,
    pub name: String,
    pub slug: Option<Slug>,
    pub description: Option<String>,
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
            description,
            url,
            public,
            private,
            backend,
        } = create;
        Ok(Self {
            org,
            name,
            slug: if let Some(slug) = slug {
                Some(Slug::from_str(&slug)?)
            } else {
                None
            },
            description,
            url: map_url(url)?,
            public: Some(if public { true } else { !private }),
            backend: backend.try_into()?,
        })
    }
}

pub fn map_url(url: Option<String>) -> Result<Option<Url>, url::ParseError> {
    Ok(if let Some(url) = url {
        Some(Url::parse(&url)?)
    } else {
        None
    })
}

impl From<Create> for JsonNewProject {
    fn from(create: Create) -> Self {
        let Create {
            org: _,
            name,
            slug,
            description,
            url,
            public,
            backend: _,
        } = create;
        Self {
            name,
            slug,
            description,
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
