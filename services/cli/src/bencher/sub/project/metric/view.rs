use bencher_json::{MetricUuid, ResourceId};

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::metric::CliMetricView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub metric: MetricUuid,
    pub backend: PubBackend,
}

impl TryFrom<CliMetricView> for View {
    type Error = CliError;

    fn try_from(view: CliMetricView) -> Result<Self, Self::Error> {
        let CliMetricView {
            project,
            metric,
            backend,
        } = view;
        Ok(Self {
            project,
            metric,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for View {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_metric_get()
                    .project(self.project.clone())
                    .metric(self.metric)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
