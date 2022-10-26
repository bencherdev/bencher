use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonNewProject, ResourceId};
use url::Url;

use crate::{
    bencher::{backend::Backend, sub::SubCmd, wide::Wide},
    cli::project::CliProjectCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub org: ResourceId,
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub url: Option<Url>,
    pub public: bool,
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
            backend,
        } = create;
        Ok(Self {
            org,
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
    async fn exec(&self, _wide: &Wide) -> Result<(), CliError> {
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
