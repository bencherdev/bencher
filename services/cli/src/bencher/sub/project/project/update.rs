use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonEmpty, JsonUpdateProject};
use bencher_json::{JsonMaybe, JsonProject, NonEmpty, ResourceId, Slug, Url};

use crate::{
    bencher::{
        backend::Backend,
        sub::{project::project::visibility::Visibility, SubCmd},
    },
    parser::project::CliProjectUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub org: Option<ResourceId>,
    pub project: ResourceId,
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub url: Option<Option<Url>>,
    pub visibility: Option<Visibility>,
    pub backend: Backend,
}

impl TryFrom<CliProjectUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliProjectUpdate) -> Result<Self, Self::Error> {
        let CliProjectUpdate {
            org,
            project,
            name,
            slug,
            url,
            visibility,
            backend,
        } = create;
        Ok(Self {
            org,
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
    fn from(create: Update) -> Self {
        let Update {
            name,
            slug,
            url,
            visibility,
            ..
        } = create;
        // Self {
        //     name: name.map(Into::into),
        //     slug: slug.map(Into::into),
        //     url: None,
        //     visibility: visibility.map(Into::into),
        // }
        Self {
            subtype_0: None,
            subtype_1: None,
        }
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonProject = self
            .backend
            .send_with(
                |client| async move {
                    if let Some(org) = self.org.clone() {
                        client
                            .org_project_patch()
                            .organization(org)
                            .project(self.project.clone())
                            .body(self.clone())
                            .send()
                            .await
                    } else {
                        let body = JsonUpdateProject::from(self.clone());
                        println!("{:?}", serde_json::to_string_pretty(&body).unwrap());
                        client
                            .project_patch()
                            .project(self.project.clone())
                            .body(body)
                            .send()
                            .await
                    }
                },
                true,
            )
            .await?;
        Ok(())
    }
}
