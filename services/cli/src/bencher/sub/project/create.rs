use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonNewProject;
use url::Url;

use super::PROJECTS_PATH;
use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::CliProjectCreate,
    BencherError,
};

#[derive(Debug)]
pub struct Project {
    pub name:        String,
    pub slug:        Option<String>,
    pub description: Option<String>,
    pub url:         Option<Url>,
    pub public:      bool,
    pub backend:     Backend,
}

impl TryFrom<CliProjectCreate> for Project {
    type Error = BencherError;

    fn try_from(create: CliProjectCreate) -> Result<Self, Self::Error> {
        let CliProjectCreate {
            name,
            slug,
            description,
            url,
            public,
            backend,
        } = create;
        Ok(Self {
            name,
            slug,
            description,
            url: map_url(url)?,
            public,
            backend: Backend::try_from(backend)?,
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

#[async_trait]
impl SubCmd for Project {
    async fn exec(&self, _wide: &Wide) -> Result<(), BencherError> {
        let project = JsonNewProject {
            name:        self.name.clone(),
            slug:        self.slug.clone(),
            description: self.description.clone(),
            url:         self.url.clone(),
            public:      self.public,
        };
        self.backend.post(PROJECTS_PATH, &project).await?;
        Ok(())
    }
}
