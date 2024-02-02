use bencher_api::{
    config::{config_tx::ConfigTx, Config},
    API_VERSION,
};
#[cfg(feature = "sentry")]
use sentry::ClientInitGuard;
use slog::{error, info, Logger};
use tokio::task::JoinHandle;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    Config(bencher_api::config::ConfigError),
    #[error("Failed to convert Bencher config to Litestream config. This is likely a bug. Please report it: {0}")]
    LitestreamYaml(serde_yaml::Error),
    #[error("Failed to write Litestream config: {0}")]
    WriteLitestreamYaml(std::io::Error),
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

#[allow(clippy::too_many_lines)]
async fn run(
    log: &Logger,
    #[cfg(feature = "sentry")] mut _guard: ClientInitGuard,
) -> Result<(), ApiError> {
    loop {
        let config = Config::load_or_default(log)
            .await
            .map_err(ApiError::Config)?;

        #[cfg(all(feature = "plus", feature = "sentry"))]
        let _guard = config
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
            });

        let (restart_tx, mut restart_rx) = tokio::sync::mpsc::channel(1);

        #[cfg(feature = "plus")]
        if let Some(litestream) = config
            .plus
            .as_ref()
            .and_then(|plus| plus.litestream.clone())
        {
            use std::path::{Path, PathBuf};

            let db_path = config.database.file.clone();
            let config_path = db_path
                .parent()
                .map_or(PathBuf::from("/"), Path::to_path_buf)
                .join("litestream.yml");
            let yaml = litestream
                .into_yaml(db_path.clone(), config.logging.log.level())
                .map_err(ApiError::LitestreamYaml)?;
            std::fs::write(&config_path, &yaml).map_err(ApiError::WriteLitestreamYaml)?;

            let restart_tx = restart_tx.clone();
            let mut litestream_handle = tokio::spawn(async move {
                // restart_tx.send(()).await;
                // litestream restore -no-expand-env -if-replica-exists -o "$LITESTREAM_DB_PATH" "$LITESTREAM_REPLICA_URL"
                tokio::process::Command::new("litestream")
                    .arg("restore")
                    .arg("-if-replica-exists")
                    .arg("-if-db-not-exists")
                    .arg("-config")
                    .arg(config_path)
                    .arg("-no-expand-env")
                    .arg(db_path)
                    .spawn()
                    .map_err(|e| ApiError::RunServer(e.to_string()))
                    .unwrap();
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5 * 60)).await;
                    // restart_tx.send(()).await.unwrap();
                }
                Ok(())
            });
            // TODO create a channel that is sent to let us know that litestream has started
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            let mut api_handle = run_api_server(config, restart_tx);
            tokio::select! {
                _ = tokio::signal::ctrl_c() => return Ok(()),
                restart = restart_rx.recv() => {
                    if restart.is_some() {
                        litestream_handle.abort();
                        api_handle.abort();
                        continue;
                    }
                    return Err(ApiError::EmptyShutdown);
                },
                result = &mut litestream_handle => {
                    return match result {
                        Ok(result) => result,
                        Err(e) => Err(ApiError::JoinHandle(e))
                    };
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

fn run_api_server(
    config: Config,
    restart_tx: tokio::sync::mpsc::Sender<()>,
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
