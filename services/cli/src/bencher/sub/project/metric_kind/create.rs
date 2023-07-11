use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::JsonNewMetricKind;
use bencher_json::{NonEmpty, ResourceId, Slug};

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
        self.backend
            .send_with(
                |client| async move {
                    client
                        .proj_metric_kind_post()
                        .project(self.project.clone())
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
