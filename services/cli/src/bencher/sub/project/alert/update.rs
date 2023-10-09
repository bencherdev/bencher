use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonAlertStatus, JsonUpdateAlert};
use bencher_json::{AlertUuid, JsonAlert, ResourceId};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::alert::{CliAlertStatus, CliAlertUpdate},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub alert: AlertUuid,
    pub status: Option<Status>,
    pub backend: Backend,
}

#[derive(Debug, Clone)]
pub enum Status {
    Active,
    Dismissed,
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

impl From<CliAlertStatus> for Status {
    fn from(status: CliAlertStatus) -> Self {
        match status {
            CliAlertStatus::Active => Self::Active,
            CliAlertStatus::Dismissed => Self::Dismissed,
        }
    }
}

impl From<Update> for JsonUpdateAlert {
    fn from(update: Update) -> Self {
        let Update { status, .. } = update;
        Self {
            status: status.map(|s| match s {
                Status::Active => JsonAlertStatus::Active,
                Status::Dismissed => JsonAlertStatus::Dismissed,
            }),
        }
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _json: JsonAlert = self
            .backend
            .send_with(
                |client| async move {
                    client
                        .proj_alert_patch()
                        .project(self.project.clone())
                        .alert(self.alert)
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
