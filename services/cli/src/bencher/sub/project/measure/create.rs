use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonNewMeasure;
use bencher_json::{JsonMeasure, NonEmpty, ResourceId, Slug};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::measure::CliMeasureCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub name: NonEmpty,
    pub slug: Option<Slug>,
    pub units: NonEmpty,
    pub backend: Backend,
}

impl TryFrom<CliMeasureCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliMeasureCreate) -> Result<Self, Self::Error> {
        let CliMeasureCreate {
            project,
            name,
            slug,
            units,
            backend,
        } = create;
        Ok(Self {
            project,
            name,
            slug,
            units,
            backend: backend.try_into()?,
        })
    }
}

impl From<Create> for JsonNewMeasure {
    fn from(create: Create) -> Self {
        let Create {
            name, slug, units, ..
        } = create;
        Self {
            name: name.into(),
            slug: slug.map(Into::into),
            units: units.into(),
        }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonMeasure = self
            .backend
            .send_with(|client| async move {
                client
                    .proj_measure_post()
                    .project(self.project.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
