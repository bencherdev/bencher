use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_json::{system::backup::JsonDataStore, JsonBackup};

use crate::{
    bencher::{backend::Backend, sub::SubCmd},
    cli::system::server::{CliBackup, CliBackupDataStore},
    CliError,
};

const BACKUP_PATH: &str = "/v0/server/backup";

#[derive(Debug, Clone)]
pub struct Backup {
    pub compress: Option<bool>,
    pub data_store: Option<BackupDataStore>,
    pub rm: Option<bool>,
    pub backend: Backend,
}

#[derive(Debug, Clone, Copy)]
pub enum BackupDataStore {
    AwsS3,
}

impl TryFrom<CliBackup> for Backup {
    type Error = CliError;

    fn try_from(create: CliBackup) -> Result<Self, Self::Error> {
        let CliBackup {
            compress,
            data_store,
            rm,
            backend,
        } = create;
        Ok(Self {
            compress: Some(compress),
            data_store: data_store.map(Into::into),
            rm: Some(rm),
            backend: backend.try_into()?,
        })
    }
}

impl From<CliBackupDataStore> for BackupDataStore {
    fn from(data_store: CliBackupDataStore) -> Self {
        match data_store {
            CliBackupDataStore::AwsS3 => Self::AwsS3,
        }
    }
}

impl From<Backup> for JsonBackup {
    fn from(backup: Backup) -> Self {
        let Backup {
            compress,
            data_store,
            rm,
            ..
        } = backup;
        Self {
            compress,
            rm,
            data_store: data_store.map(Into::into),
        }
    }
}

impl From<BackupDataStore> for JsonDataStore {
    fn from(data_store: BackupDataStore) -> Self {
        match data_store {
            BackupDataStore::AwsS3 => Self::AwsS3,
        }
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
