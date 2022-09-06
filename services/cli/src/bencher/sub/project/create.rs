use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonNewProject, ResourceId};
use url::Url;

use super::PROJECTS_PATH;
use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::project::CliProjectCreate,
    BencherError,
};

#[derive(Debug, Clone)]
pub struct Project {
    pub organization: ResourceId,
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub url: Option<Url>,
    pub public: bool,
    pub backend: Backend,
}

impl TryFrom<CliProjectCreate> for Project {
    type Error = BencherError;

    fn try_from(create: CliProjectCreate) -> Result<Self, Self::Error> {
        let CliProjectCreate {
            organization,
            name,
            slug,
            description,
            url,
            public,
            backend,
        } = create;
        Ok(Self {
            organization,
            name,
            slug,
            description,
            url: map_url(url)?,
            public,
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

impl Into<JsonNewProject> for Project {
    fn into(self) -> JsonNewProject {
        let Self {
            organization,
            name,
            slug,
            description,
            url,
            public,
            backend: _,
        } = self;
        JsonNewProject {
            organization,
            name,
            slug,
            description,
            url,
            public,
        }
    }
}

#[async_trait]
impl SubCmd for Project {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        let project: JsonNewProject = self.clone().into();
        self.backend.post(PROJECTS_PATH, &project).await?;
        Ok(())
    }
}
