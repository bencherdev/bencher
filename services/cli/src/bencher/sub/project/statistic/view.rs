use bencher_json::{ResourceId, StatisticUuid};

use crate::{
    bencher::{backend::PubBackend, sub::SubCmd},
    parser::project::statistic::CliStatisticView,
    CliError,
};

#[derive(Debug)]
pub struct View {
    pub project: ResourceId,
    pub statistic: StatisticUuid,
    pub backend: PubBackend,
}

impl TryFrom<CliStatisticView> for View {
    type Error = CliError;

    fn try_from(view: CliStatisticView) -> Result<Self, Self::Error> {
        let CliStatisticView {
            project,
            statistic,
            backend,
        } = view;
        Ok(Self {
            project,
            statistic,
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
                    .proj_statistic_get()
                    .project(self.project.clone())
                    .statistic(self.statistic)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
