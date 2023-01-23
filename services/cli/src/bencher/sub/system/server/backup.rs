use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::JsonBackup;

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::server::CliBackup,
    CliError,
};

const BACKUP_PATH: &str = "/v0/server/backup";

#[derive(Debug, Clone)]
pub struct Backup {
    pub vacuum: Option<bool>,
    pub backend: Backend,
}

impl TryFrom<CliBackup> for Backup {
    type Error = CliError;

    fn try_from(create: CliBackup) -> Result<Self, Self::Error> {
        let CliBackup { exact, backend } = create;
        Ok(Self {
            vacuum: Some(!exact),
            backend: backend.try_into()?,
        })
    }
}

impl From<Backup> for JsonBackup {
    fn from(backup: Backup) -> Self {
        let Backup { vacuum, .. } = backup;
        Self { vacuum }
    }
}

#[async_trait]
impl SubCmd for Backup {
    async fn exec(&self) -> Result<(), CliError> {
        let backup: JsonBackup = self.clone().into();
        self.backend.post(BACKUP_PATH, &backup).await?;
        Ok(())
    }
}
