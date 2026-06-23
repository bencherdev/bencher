use camino::Utf8PathBuf;
use slog::Logger;
use tokio::{process::Command, sync::oneshot, task::JoinHandle};

use crate::{JsonLitestream, LitestreamLevel};

/// Render the Litestream config to `config_path`, then supervise `litestream restore`
/// followed by `litestream replicate` around the `SQLite` database at `db_path`.
///
/// `db_path` must be absolute. Once the one-shot restore completes, `restore_tx` is
/// signaled (the database file now exists) and replication continues on the returned
/// [`JoinHandle`] for the lifetime of the process. The `litestream` binary is resolved
/// on `PATH`.
pub fn run_litestream(
    log: &Logger,
    litestream: JsonLitestream,
    db_path: Utf8PathBuf,
    config_path: Utf8PathBuf,
    log_level: LitestreamLevel,
    restore_tx: oneshot::Sender<()>,
) -> Result<JoinHandle<Result<(), LitestreamError>>, LitestreamError> {
    let yaml = litestream
        .into_yaml(db_path.clone(), log_level)
        .map_err(LitestreamError::Yaml)?;
    std::fs::write(&config_path, yaml)
        .map_err(|e| LitestreamError::WriteYaml(config_path.clone(), e))?;

    let litestream_logger = log.clone();
    Ok(tokio::spawn(async move {
        // https://litestream.io/reference/restore/
        let restore = Command::new("litestream")
            .arg("restore")
            .arg("-if-replica-exists")
            .arg("-if-db-not-exists")
            .arg("-config")
            .arg(&config_path)
            .arg("-no-expand-env")
            .arg(&db_path)
            .output()
            .await
            .map_err(LitestreamError::Restore)?;
        slog::info!(litestream_logger, "Litestream restore: {restore:?}");
        if !restore.status.success() {
            return Err(LitestreamError::RestoreExit {
                status: restore.status,
                stdout: String::from_utf8_lossy(&restore.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&restore.stderr).into_owned(),
            });
        }

        // Signal the caller that restore is complete (the DB file exists)
        restore_tx.send(()).map_err(LitestreamError::RestoreSend)?;

        // https://litestream.io/reference/replicate/
        let mut replicate = Command::new("litestream")
            .arg("replicate")
            .arg("-config")
            .arg(&config_path)
            .arg("-no-expand-env")
            .spawn()
            .map_err(LitestreamError::Replicate)?;
        // Litestream should run indefinitely
        Err(LitestreamError::ReplicateExit(
            replicate.wait().await.map_err(LitestreamError::Replicate)?,
        ))
    }))
}

#[derive(Debug, thiserror::Error)]
pub enum LitestreamError {
    #[error("Database path is not valid UTF-8: {0:?}")]
    DatabasePathNotUtf8(std::path::PathBuf),
    #[error("Failed to absolutize the database path: {0}")]
    Database(std::io::Error),
    #[error(
        "Failed to convert Bencher config to Litestream config. This is likely a bug. Please report this: {0}"
    )]
    Yaml(serde_yaml::Error),
    #[error("Failed to write Litestream config ({0}): {1}")]
    WriteYaml(Utf8PathBuf, std::io::Error),
    #[error("Failed to run `litestream restore`: {0}")]
    Restore(std::io::Error),
    #[error("Failed to run `litestream replicate`: {0}")]
    Replicate(std::io::Error),
    #[error("Failed to restore (exit status {status})\nstdout: {stdout}\nstderr: {stderr}")]
    RestoreExit {
        status: std::process::ExitStatus,
        stdout: String,
        stderr: String,
    },
    #[error(
        "Failed to send restore completion message: receiver dropped, server likely crashed during startup"
    )]
    RestoreSend(()),
    #[error("Failed to receive restore completion message")]
    RestoreRecv(oneshot::error::RecvError),
    #[error("Failed to replicate: {0}")]
    ReplicateExit(std::process::ExitStatus),
    #[error("Failed to join Litestream handle: {0}")]
    JoinHandle(tokio::task::JoinError),
}
