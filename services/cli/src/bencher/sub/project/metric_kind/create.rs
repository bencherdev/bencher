use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{JsonNewMetricKind, NonEmpty, ResourceId, Slug};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::project::metric_kind::CliMetricKindCreate,
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

impl TryFrom<CliMetricKindCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliMetricKindCreate) -> Result<Self, Self::Error> {
        let CliMetricKindCreate {
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

impl From<Create> for JsonNewMetricKind {
    fn from(create: Create) -> Self {
        let Create {
            name, slug, units, ..
        } = create;
        Self { name, slug, units }
    }
}

#[async_trait]
impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let metric_kind: JsonNewMetricKind = self.clone().into();
        self.backend
            .post(
                &format!("/v0/projects/{}/metric-kinds", self.project),
                &metric_kind,
            )
            .await?;
        Ok(())
    }
}
