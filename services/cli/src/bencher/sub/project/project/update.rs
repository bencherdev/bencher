use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{
    JsonProjectPatch, JsonProjectPatchNull, JsonUpdateProject, Visibility,
};
use bencher_json::{ResourceId, ResourceName, Slug, Url};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::CliProjectUpdate,
    CliError,
};

#[derive(Debug, Clone)]
#[allow(clippy::option_option)]
pub struct Update {
    pub project: ResourceId,
    pub name: Option<ResourceName>,
    pub slug: Option<Slug>,
    pub url: Option<Option<Url>>,
    pub visibility: Option<Visibility>,
    pub backend: AuthBackend,
}

impl TryFrom<CliProjectUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliProjectUpdate) -> Result<Self, Self::Error> {
        let CliProjectUpdate {
            project,
            name,
            slug,
            url,
            visibility,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug,
            url,
            visibility: visibility.map(Into::into),
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateProject {
    fn from(update: Update) -> Self {
        let Update {
            name,
            slug,
            url,
            visibility,
            ..
        } = update;
        match url {
            Some(Some(url)) => Self {
                subtype_0: Some(JsonProjectPatch {
                    name: name.map(Into::into),
                    slug: slug.map(Into::into),
                    url: Some(url.into()),
                    visibility,
                }),
                subtype_1: None,
            },
            Some(None) => Self {
                subtype_0: None,
                subtype_1: Some(JsonProjectPatchNull {
                    name: name.map(Into::into),
                    slug: slug.map(Into::into),
                    url: (),
                    visibility,
                }),
            },
            None => Self {
                subtype_0: Some(JsonProjectPatch {
                    name: name.map(Into::into),
                    slug: slug.map(Into::into),
                    url: None,
                    visibility,
                }),
                subtype_1: None,
            },
        }
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .project_patch()
                    .project(self.project.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
