use bencher_client::types::{JsonUpdateChangepoint, UpdateChangepointStatus};
use bencher_json::{ChangepointUuid, ProjectResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::changepoint::{CliChangepointStatusUpdate, CliChangepointUpdate},
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ProjectResourceId,
    pub changepoint: ChangepointUuid,
    pub status: UpdateChangepointStatus,
    pub backend: AuthBackend,
}

impl TryFrom<CliChangepointUpdate> for Update {
    type Error = CliError;

    fn try_from(update: CliChangepointUpdate) -> Result<Self, Self::Error> {
        let CliChangepointUpdate {
            project,
            changepoint,
            status,
            backend,
        } = update;
        Ok(Self {
            project,
            changepoint,
            status: status.into(),
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateChangepoint {
    fn from(update: Update) -> Self {
        let Update { status, .. } = update;
        Self { status }
    }
}

impl From<CliChangepointStatusUpdate> for UpdateChangepointStatus {
    fn from(status: CliChangepointStatusUpdate) -> Self {
        match status {
            CliChangepointStatusUpdate::Active => Self::Active,
            CliChangepointStatusUpdate::Dismissed => Self::Dismissed,
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_changepoint_patch()
                    .project(self.project.clone())
                    .changepoint(self.changepoint)
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
