use bencher_json::{ProjectResourceId, ThresholdUuid};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::threshold::CliThresholdDelete,
};

#[derive(Debug)]
pub struct Delete {
    pub project: ProjectResourceId,
    pub threshold: ThresholdUuid,
    pub backend: AuthBackend,
}

impl TryFrom<CliThresholdDelete> for Delete {
    type Error = CliError;

    fn try_from(delete: CliThresholdDelete) -> Result<Self, Self::Error> {
        let CliThresholdDelete {
            project,
            threshold,
            backend,
        } = delete;
        Ok(Self {
            project,
            threshold,
            backend: backend.try_into()?,
        })
    }
}

impl SubCmd for Delete {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_threshold_delete()
                    .project(self.project.clone())
                    .threshold(self.threshold)
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
