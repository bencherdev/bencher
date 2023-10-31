use bencher_api::{
    config::{config_tx::ConfigTx, Config},
    API_VERSION,
};
use dropshot::HttpServer;
#[cfg(feature = "sentry")]
use sentry::ClientInitGuard;
use slog::{error, info, Logger};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    Config(bencher_api::config::ConfigError),
    #[error("{0}")]
    ConfigTxError(bencher_api::config::config_tx::ConfigTxError),
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
        let _guard = config
            .as_ref()
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
        let config_tx = ConfigTx { config, restart_tx };

        let handle = tokio::spawn(async move {
            config_tx
                .into_server()
                .await
                .map_err(ApiError::ConfigTxError)?
                .await
                .map_err(ApiError::RunServer)
        });

        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            restart = restart_rx.recv() => {
                if restart.is_some() {
                    handle.abort();
                } else {
                    break;
                }
            },
            () = async {}, if handle.is_finished() => {
                return match handle.await {
                    Ok(result) => result,
                    Err(e) => Err(ApiError::JoinHandle(e))
                };
            },
        }
    }

    Ok(())
}
