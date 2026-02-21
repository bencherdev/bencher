#[cfg(feature = "plus")]
use bencher_client::types::JsonTestbedPatchNull;
use bencher_client::types::{JsonTestbedPatch, JsonUpdateTestbed};
#[cfg(feature = "plus")]
use bencher_json::SpecResourceId;
use bencher_json::{ProjectResourceId, ResourceName, TestbedResourceId, TestbedSlug};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::testbed::CliTestbedUpdate,
};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "plus", expect(clippy::option_option))]
pub struct Update {
    pub project: ProjectResourceId,
    pub testbed: TestbedResourceId,
    pub name: Option<ResourceName>,
    pub slug: Option<TestbedSlug>,
    #[cfg(feature = "plus")]
    pub spec: Option<Option<SpecResourceId>>,
    pub archived: Option<bool>,
    pub backend: AuthBackend,
}

impl TryFrom<CliTestbedUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliTestbedUpdate) -> Result<Self, Self::Error> {
        let CliTestbedUpdate {
            project,
            testbed,
            name,
            slug,
            #[cfg(feature = "plus")]
            spec,
            archived,
            backend,
        } = create;
        Ok(Self {
            project,
            testbed,
            name,
            slug,
            #[cfg(feature = "plus")]
            spec: spec.map(Into::into),
            archived: archived.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateTestbed {
    fn from(update: Update) -> Self {
        let Update {
            name,
            slug,
            #[cfg(feature = "plus")]
            spec,
            archived,
            ..
        } = update;
        #[cfg(feature = "plus")]
        match spec {
            Some(Some(spec)) => Self {
                subtype_0: Some(JsonTestbedPatch {
                    name: name.map(Into::into),
                    slug: slug.map(Into::into),
                    spec: Some(spec.into()),
                    archived,
                }),
                subtype_1: None,
            },
            Some(None) => Self {
                subtype_0: None,
                subtype_1: Some(JsonTestbedPatchNull {
                    name: name.map(Into::into),
                    slug: slug.map(Into::into),
                    spec: (),
                    archived,
                }),
            },
            None => Self {
                subtype_0: Some(JsonTestbedPatch {
                    name: name.map(Into::into),
                    slug: slug.map(Into::into),
                    spec: None,
                    archived,
                }),
                subtype_1: None,
            },
        }
        #[cfg(not(feature = "plus"))]
        Self {
            subtype_0: Some(JsonTestbedPatch {
                name: name.map(Into::into),
                slug: slug.map(Into::into),
                spec: None,
                archived,
            }),
            subtype_1: None,
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_testbed_patch()
                    .project(self.project.clone())
                    .testbed(self.testbed.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
