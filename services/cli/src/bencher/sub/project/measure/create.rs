use bencher_client::types::JsonNewMeasure;
use bencher_json::{ResourceId, ResourceName, Slug};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::measure::CliMeasureCreate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub project: ResourceId,
    pub name: ResourceName,
    pub slug: Option<Slug>,
    pub units: ResourceName,
    pub backend: AuthBackend,
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

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
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
