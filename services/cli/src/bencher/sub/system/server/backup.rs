use std::convert::TryFrom;

use async_trait::async_trait;
use bencher_client::types::{JsonBackup, JsonDataStore};

use crate::{
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::server::{CliBackup, CliBackupDataStore},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Backup {
    pub compress: Option<bool>,
    pub data_store: Option<JsonDataStore>,
    pub rm: Option<bool>,
    pub backend: AuthBackend,
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

impl From<CliBackupDataStore> for JsonDataStore {
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
            data_store,
            rm,
        }
    }
}

#[async_trait]
impl SubCmd for Backup {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .as_ref()
            .send(
                |client| async move { client.server_backup_post().body(self.clone()).send().await },
            )
            .await?;
        Ok(())
    }
}
