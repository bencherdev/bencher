use std::{ffi::OsStr, path::PathBuf};

use async_compression::tokio::write::GzipEncoder;
use bencher_json::{DateTime, JsonBackup, JsonBackupCreated, system::backup::JsonDataStore};
use chrono::Utc;
use tokio::{
    fs::remove_file,
    io::{AsyncReadExt as _, AsyncWriteExt as _, BufReader, BufWriter},
};

use crate::context::DataStore;

// https://www.sqlite.org/pgszchng2016.html
const SQLITE_PAGE_SIZE: usize = 4096;
const PAUSE_BETWEEN_PAGES: std::time::Duration = std::time::Duration::from_millis(100);
// We want the backup to be done in a reasonable time, ~10 minutes.
// Sqlite page size * 10 minutes * 60 seconds * 10 iterations of 100 milliseconds pauses per second
const PAGES_PER_STEP_COEFFICIENT: usize = SQLITE_PAGE_SIZE * 10 * 60 * 10;

pub struct ServerBackup {
    file_path: PathBuf,
    file_name: String,
    created: DateTime,
}

#[derive(Debug, thiserror::Error)]
pub enum ServerBackupError {
    #[error("Failed to get source database size ({path}): {error}")]
    GetSourceDatabaseSize {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("Failed to open source database ({path}): {error}")]
    OpenSourceDatabase {
        path: PathBuf,
        error: rusqlite::Error,
    },
    #[error("Failed to open destination database ({path}): {error}")]
    OpenDestinationDatabase {
        path: PathBuf,
        error: rusqlite::Error,
    },
    #[error("Failed to create online backup: {0}")]
    CreateBackup(rusqlite::Error),
    #[error("Failed to run online backup: {0}")]
    RunBackup(rusqlite::Error),
    #[error("Failed to join backup task: {0}")]
    JoinBackup(tokio::task::JoinError),
    #[error("Failed to open backup file: {0}")]
    OpenBackupFile(std::io::Error),
    #[error("Failed to create compressed file: {0}")]
    CreateZipFile(std::io::Error),
    #[error("Failed to write to compressed file: {0}")]
    WriteZipFile(std::io::Error),
    #[error("Failed to close compressed file: {0}")]
    CloseZipFile(std::io::Error),
    #[error("Failed to remove backup file: {0}")]
    RmBackupFile(std::io::Error),
    #[error("Failed to remove compressed file: {0}")]
    RmZipFile(std::io::Error),
    #[error("{0}")]
    DataStore(crate::context::DataStoreError),
    #[error("No data store")]
    NoDataStore,
}

impl ServerBackup {
    pub async fn run(
        file_path: PathBuf,
        data_store: Option<&DataStore>,
        json_backup: JsonBackup,
    ) -> Result<JsonBackupCreated, ServerBackupError> {
        // Create a database backup
        let Self {
            file_path: backup_file_path,
            file_name: backup_file_name,
            created,
        } = Self::backup_database(file_path).await?;

        // Compress the database backup
        let compress = json_backup.compress.unwrap_or_default();
        let (source_path, file_name) = if compress {
            compress_database(backup_file_path, &backup_file_name).await?
        } else {
            (backup_file_path, backup_file_name)
        };

        // Store the database backup in AWS S3
        if let Some(JsonDataStore::AwsS3) = json_backup.data_store {
            if let Some(data_store) = data_store {
                data_store
                    .backup(&source_path, &file_name)
                    .await
                    .map_err(ServerBackupError::DataStore)?;
            } else {
                return Err(ServerBackupError::NoDataStore);
            }
        }

        // Remove the remaining database backup
        if json_backup.rm.unwrap_or_default() {
            remove_file(source_path).await.map_err(|e| {
                if compress {
                    ServerBackupError::RmZipFile(e)
                } else {
                    ServerBackupError::RmBackupFile(e)
                }
            })?;
        }

        Ok(JsonBackupCreated { created })
    }

    async fn backup_database(file_path: PathBuf) -> Result<Self, ServerBackupError> {
        let file_stem = file_path
            .file_stem()
            .unwrap_or_else(|| OsStr::new("bencher"))
            .to_string_lossy();
        let file_extension = file_path
            .extension()
            .unwrap_or_else(|| OsStr::new("db"))
            .to_string_lossy();
        let date_time = Utc::now();
        let file_name = format!(
            "backup-{file_stem}-{}.{file_extension}",
            date_time.format("%Y-%m-%d-%H-%M-%S")
        );
        let mut backup_file_path = file_path.clone();
        backup_file_path.set_file_name(&file_name);

        let dest = backup_file_path.clone();
        tokio::task::spawn_blocking(move || run_online_backup(&file_path, &dest))
            .await
            .map_err(ServerBackupError::JoinBackup)??;

        Ok(Self {
            file_path: backup_file_path,
            file_name,
            created: date_time.into(),
        })
    }
}

fn run_online_backup(src: &PathBuf, dest: &PathBuf) -> Result<(), ServerBackupError> {
    // Get the total size of the source database
    let src_size = src
        .metadata()
        .map_err(|error| ServerBackupError::GetSourceDatabaseSize {
            path: src.clone(),
            error,
        })?
        .len();
    // Calculate the number of pages per step in order to complete the backup in a reasonable time
    #[expect(
        clippy::cast_possible_truncation,
        clippy::integer_division,
        reason = "precision is not needed"
    )]
    let pages_per_step = (src_size / PAGES_PER_STEP_COEFFICIENT as u64) as i32;

    let src_connection =
        rusqlite::Connection::open(src).map_err(|error| ServerBackupError::OpenSourceDatabase {
            path: src.clone(),
            error,
        })?;
    let mut dest_connection = rusqlite::Connection::open(dest).map_err(|error| {
        ServerBackupError::OpenDestinationDatabase {
            path: dest.clone(),
            error,
        }
    })?;
    let backup = rusqlite::backup::Backup::new(&src_connection, &mut dest_connection)
        .map_err(ServerBackupError::CreateBackup)?;
    // https://www.sqlite.org/backup.html#example_2_online_backup_of_a_running_database
    backup
        .run_to_completion(pages_per_step, PAUSE_BETWEEN_PAGES, None)
        .map_err(ServerBackupError::RunBackup)
}

async fn compress_database(
    backup_file_path: PathBuf,
    backup_file_name: &str,
) -> Result<(PathBuf, String), ServerBackupError> {
    let backup_file = tokio::fs::File::open(&backup_file_path)
        .await
        .map_err(ServerBackupError::OpenBackupFile)?;
    let mut backup_data = BufReader::with_capacity(SQLITE_PAGE_SIZE, backup_file);

    let compress_file_name = format!("{backup_file_name}.gz");
    let mut compress_file_path = backup_file_path.clone();
    compress_file_path.set_file_name(&compress_file_name);
    let compress_file = tokio::fs::File::create(&compress_file_path)
        .await
        .map_err(ServerBackupError::CreateZipFile)?;
    let compress_data = BufWriter::with_capacity(SQLITE_PAGE_SIZE, compress_file);

    let mut encoder = GzipEncoder::new(compress_data);
    let mut data_buffer = [0; SQLITE_PAGE_SIZE];
    while let Ok(data_size) = backup_data.read(&mut data_buffer).await {
        if data_size == 0 {
            break;
        }

        encoder
            .write_all(&data_buffer)
            .await
            .map_err(ServerBackupError::WriteZipFile)?;
    }
    encoder
        .shutdown()
        .await
        .map_err(ServerBackupError::CloseZipFile)?;

    remove_file(backup_file_path)
        .await
        .map_err(ServerBackupError::RmBackupFile)?;

    Ok((compress_file_path, compress_file_name))
}
