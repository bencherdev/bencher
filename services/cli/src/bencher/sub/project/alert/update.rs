use bencher_client::types::{JsonUpdateAlert, UpdateAlertStatus};
use bencher_json::{AlertUuid, ResourceId};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::project::alert::{CliAlertStatusUpdate, CliAlertUpdate},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub alert: AlertUuid,
    pub status: Option<UpdateAlertStatus>,
    pub backend: AuthBackend,
}

impl TryFrom<CliAlertUpdate> for Update {
    type Error = CliError;

    fn try_from(create: CliAlertUpdate) -> Result<Self, Self::Error> {
        let CliAlertUpdate {
            project,
            alert,
            status,
            backend,
        } = create;
        Ok(Self {
            project,
            alert,
            status: status.map(Into::into),
            backend: backend.try_into()?,
        })
    }
}

impl From<Update> for JsonUpdateAlert {
    fn from(update: Update) -> Self {
        let Update { status, .. } = update;
        Self { status }
    }
}

impl From<CliAlertStatusUpdate> for UpdateAlertStatus {
    fn from(status: CliAlertStatusUpdate) -> Self {
        match status {
            CliAlertStatusUpdate::Active => Self::Active,
            CliAlertStatusUpdate::Dismissed => Self::Dismissed,
        }
    }
}

impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .proj_alert_patch()
                    .project(self.project.clone())
                    .alert(self.alert)
                    .body(self.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}
