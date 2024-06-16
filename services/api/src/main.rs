#[cfg(feature = "sentry")]
use std::path::PathBuf;

use bencher_api::{
    config::{config_tx::ConfigTx, Config},
    API_VERSION,
};
#[cfg(feature = "plus")]
use bencher_json::system::config::JsonLitestream;
#[cfg(feature = "sentry")]
use sentry::ClientInitGuard;
use slog::{error, info, Logger};
#[cfg(feature = "plus")]
use tokio::process::Command;
use tokio::{sync, task::JoinHandle};

#[allow(clippy::absolute_paths)]
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    Config(bencher_api::config::ConfigError),
    #[cfg(feature = "plus")]
    #[error("{0}")]
    Litestream(#[from] LitestreamError),
    #[error("{0}")]
    ConfigTxError(bencher_api::config::config_tx::ConfigTxError),
    #[error("Unexpected empty shutdown signal. This is likely a bug. Please report it.")]
    EmptyShutdown,
    #[error("Shutting down server: {0}")]
    RunServer(String),
    #[error("Failed to join handle: {0}")]
    JoinHandle(tokio::task::JoinError),
}

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    let log = bencher_logger::bootstrap_logger();
    #[cfg(feature = "sentry")]
    let guard = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    });
    info!(&log, "🐰 Bencher API Server v{API_VERSION}");
    if let Err(e) = run(
        &log,
        #[cfg(feature = "sentry")]
        guard,
    )
    .await
    {
        error!(&log, "Server failed to run: {e}");
        return Err(e);
    }
    Ok(())
}

async fn run(
    log: &Logger,
    #[cfg(feature = "sentry")] mut _guard: ClientInitGuard,
) -> Result<(), ApiError> {
    loop {
        let config = Config::load_or_default(log)
            .await
            .map_err(ApiError::Config)?;

        #[cfg(all(feature = "plus", feature = "sentry"))]
        let _guard = init_sentry(&config);

        let (restart_tx, mut restart_rx) = sync::mpsc::channel(1);
        #[cfg(feature = "plus")]
        if let Some(litestream) = config
            .plus
            .as_ref()
            .and_then(|plus| plus.litestream.clone())
        {
            let (replicate_tx, replicate_rx) = sync::oneshot::channel();
            let mut litestream_handle = run_litestream(log, &config, litestream, replicate_tx)?;
            // Wait for Litestream to start replicating
            replicate_rx.await.map_err(LitestreamError::ReplicateRecv)?;

            let mut api_handle = run_api_server(config, restart_tx);
            tokio::select! {
                _ = tokio::signal::ctrl_c() => return Ok(()),
                restart = restart_rx.recv() => {
                    if restart.is_some() {
                        api_handle.abort();
                        litestream_handle.abort();
                        continue;
                    }
                    return Err(ApiError::EmptyShutdown);
                },
                result = &mut litestream_handle => {
                    return match result {
                        Ok(result) => result,
                        Err(e) => Err(LitestreamError::JoinHandle(e))
                    }.map_err(Into::into);
                },
                result = &mut api_handle => {
                    return match result {
                        Ok(result) => result,
                        Err(e) => Err(ApiError::JoinHandle(e))
                    };
                },
            }
        }

        let mut api_handle = run_api_server(config, restart_tx);
        tokio::select! {
            _ = tokio::signal::ctrl_c() => return Ok(()),
            restart = restart_rx.recv() => {
                if restart.is_some() {
                    api_handle.abort();
                    continue;
                }
                return Err(ApiError::EmptyShutdown);
            },
            result = &mut api_handle => {
                return match result {
                    Ok(result) => result,
                    Err(e) => Err(ApiError::JoinHandle(e))
                };
            },
        }
    }
}

#[cfg(all(feature = "plus", feature = "sentry"))]
fn init_sentry(config: &Config) -> Option<ClientInitGuard> {
    config
        .plus
        .as_ref()
        .and_then(|plus| plus.cloud.as_ref())
        .and_then(|cloud| cloud.sentry.as_ref())
        .map(|sentry_dsn| {
            sentry::init((
                sentry_dsn.as_ref(),
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    ..Default::default()
                },
            ))
        })
}

#[cfg(feature = "plus")]
#[allow(clippy::absolute_paths)]
#[derive(Debug, thiserror::Error)]
pub enum LitestreamError {
    #[error("Failed to absolutize the database path: {0}")]
    Database(std::io::Error),
    #[error("Failed to convert Bencher config to Litestream config. This is likely a bug. Please report this: {0}")]
    Yaml(serde_yaml::Error),
    #[error("Failed to write Litestream config ({0}): {1}")]
    WriteYaml(PathBuf, std::io::Error),
    #[error("Failed to run `litestream restore`: {0}")]
    Restore(std::io::Error),
    #[error("Failed to run `litestream replicate`: {0}")]
    Replicate(std::io::Error),
    #[error("Failed to send replication start message")]
    ReplicateSend(()),
    #[error("Failed to receive replication start message")]
    ReplicateRecv(sync::oneshot::error::RecvError),
    #[error("Failed to replicate: {0}")]
    ReplicateExit(std::process::ExitStatus),
    #[error("Failed to join Litestream handle: {0}")]
    JoinHandle(tokio::task::JoinError),
}

#[cfg(feature = "plus")]
fn run_litestream(
    log: &Logger,
    config: &Config,
    litestream: JsonLitestream,
    replicate_tx: sync::oneshot::Sender<()>,
) -> Result<JoinHandle<Result<(), LitestreamError>>, LitestreamError> {
    // Get the absolute database path from the config
    let db_path = if config.database.file.is_absolute() {
        config.database.file.clone()
    } else {
        std::env::current_dir()
            .map_err(LitestreamError::Database)?
            .join(&config.database.file)
    };
    #[cfg(debug_assertions)]
    let config_path = PathBuf::from("etc/litestream.yml");
    #[cfg(not(debug_assertions))]
    let config_path = PathBuf::from("/etc/litestream.yml");
    let yaml = litestream
        .into_yaml(db_path.clone(), config.logging.log.level())
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

        // https://litestream.io/reference/replicate/
        let mut replicate = Command::new("litestream")
            .arg("replicate")
            .arg("-config")
            .arg(&config_path)
            .arg("-no-expand-env")
            .spawn()
            .map_err(LitestreamError::Replicate)?;
        // Let the server know that Litestream is running
        replicate_tx
            .send(())
            .map_err(LitestreamError::ReplicateSend)?;
        // Litestream should run indefinitely
        Err(LitestreamError::ReplicateExit(
            replicate.wait().await.map_err(LitestreamError::Replicate)?,
        ))
    }))
}

fn run_api_server(
    config: Config,
    restart_tx: sync::mpsc::Sender<()>,
) -> JoinHandle<Result<(), ApiError>> {
    let config_tx = ConfigTx { config, restart_tx };
    tokio::spawn(async move {
        config_tx
            .into_server()
            .await
            .map_err(ApiError::ConfigTxError)?
            .await
            .map_err(ApiError::RunServer)
    })
}
