use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonAlertStatus, JsonUpdateAlert};
use bencher_json::{JsonAlert, ResourceId};
use uuid::Uuid;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    parser::project::alert::{CliAlertStatus, CliAlertUpdate},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Update {
    pub project: ResourceId,
    pub alert: Uuid,
    pub status: Option<Status>,
    pub backend: Backend,
}

#[derive(Debug, Clone)]
pub enum Status {
    Unread,
    Read,
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
            CliAlertStatus::Unread => Self::Unread,
            CliAlertStatus::Read => Self::Read,
        }
    }
}

impl From<Update> for JsonUpdateAlert {
    fn from(create: Update) -> Self {
        let Update { status, .. } = create;
        Self {
            status: status.map(|s| match s {
                Status::Unread => JsonAlertStatus::Unread,
                Status::Read => JsonAlertStatus::Read,
            }),
        }
    }
}

#[async_trait]
impl SubCmd for Update {
    async fn exec(&self) -> Result<(), CliError> {
        let _: JsonAlert = self
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
