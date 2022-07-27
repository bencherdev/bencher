use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonProject;
use url::Url;

use crate::{
    bencher::{
        backend::Backend,
        sub::SubCmd,
        wide::Wide,
    },
    cli::CliProjectCreate,
    BencherError,
};

const PROJECTS_PATH: &str = "/v0/projects";

#[derive(Debug)]
pub struct Project {
    pub name:        String,
    pub slug:        Option<String>,
    pub description: Option<String>,
    pub url:         Option<Url>,
    pub default:     bool,
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
            default,
            backend,
        } = create;
        Ok(Self {
            name,
            slug,
            description,
            url: map_url(url)?,
            default,
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
        let project = JsonProject {
            name:        self.name.clone(),
            slug:        self.slug.clone(),
            description: self.description.clone(),
            url:         self.url.clone(),
            default:     self.default,
        };
        self.backend.post(PROJECTS_PATH, &project).await
    }
}
