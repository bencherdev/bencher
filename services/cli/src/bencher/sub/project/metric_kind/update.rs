use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonUpdateMetricKind;
use bencher_json::{JsonMetricKind, NonEmpty, ResourceId, Slug};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::metric_kind::CliMetricKindUpdate,
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub metric_kind: ResourceId,
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub units: Option<NonEmpty>,
    pub backend: Backend,
}

impl TryFrom<CliMetricKindUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliMetricKindUpdate) -> Result<Self, Self::Error> {
        let CliMetricKindUpdate {
            project,
            metric_kind,
            name,
            slug,
            units,
            backend,
        } = create;
        Ok(Self {
            project,
            metric_kind,
            name,
            slug,
            units,
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateMetricKind {
    fn from(update: Update) -> Self {
        let Update {
            name, slug, units, ..
        } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            units: units.map(Into::into),
        }
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonMetricKind = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_metric_kind_patch()
                        .project(self.project.clone())
                        .metric_kind(self.metric_kind.clone())
                        .body(self.clone())
                        .send()
                        .await
                },
                true,
            )
            .await?;
        Ok(())
    }
}
