use bencher_client::types::JsonUpdateMeasure;
use bencher_json::{MeasureSlug, ResourceId, ResourceName};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::measure::CliMeasureUpdate,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub measure: ResourceId,
    pub name: Option<ResourceName>,
    pub slug: Option<MeasureSlug>,
    pub units: Option<ResourceName>,
    pub archived: Option<bool>,
    pub backend: AuthBackend,
}

impl TryFrom<CliMeasureUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliMeasureUpdate) -> Result<Self, Self::Error> {
        let CliMeasureUpdate {
            project,
            measure,
            name,
            slug,
            units,
            archived,
            backend,
        } = create;
        Ok(Self {
            project,
            measure,
            name,
            slug,
            units,
            archived: archived.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateMeasure {
    fn from(update: Update) -> Self {
        let Update {
            name,
            slug,
            units,
            archived,
            ..
        } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            units: units.map(Into::into),
            archived,
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_measure_patch()
                    .project(self.project.clone())
                    .measure(self.measure.clone())
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
