use bencher_client::types::{JsonBackup, JsonDataStore};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::system::server::{CliBackup, CliBackupDataStore},
};

#[derive(Debug, Clone)]
pub struct Backup {
    pub compress: Option<bool>,
    pub data_store: Option<JsonDataStore>,
    pub remove: Option<bool>,
    pub backend: AuthBackend,
}

impl TryFrom<CliBackup> for Backup {
    type Error = CliError;

    fn try_from(create: CliBackup) -> Result<Self, Self::Error> {
        let CliBackup {
            compress,
            data_store,
            remove,
            backend,
        } = create;
        Ok(Self {
            compress: Some(compress),
            data_store: data_store.map(Into::into),
            remove: Some(remove),
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
            remove,
            ..
        } = backup;
        Self {
            compress,
            data_store,
            rm: remove,
        }
    }
}

impl SubCmd for Backup {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(
                |client| async move { client.server_backup_post().body(self.clone()).send().await },
            )
            .await?;
        Ok(())
    }
}
