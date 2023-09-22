use bencher_api::{
    config::{config_tx::ConfigTx, Config},
    util::logger::bootstrap_logger,
    ApiError,
};
use bencher_json::system::config::JsonApm;
use dropshot::HttpServer;
#[cfg(feature = "sentry")]
use sentry::ClientInitGuard;
use slog::{error, info, Logger};

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    let log = bootstrap_logger();
    #[cfg(feature = "sentry")]
    let guard = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    });
    info!(&log, "🐰 Bencher API Server v{}", env!("CARGO_PKG_VERSION"));
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
        let config = Config::load_or_default(log).await?;
        if let Some(apm) = config.as_ref().apm.as_ref() {
            #[allow(unused_variables)]
            match &apm {
                JsonApm::Sentry { dsn } => {
                    #[cfg(feature = "sentry")]
                    {
                        _guard = sentry::init((
                            dsn.as_str(),
                            sentry::ClientOptions {
                                release: sentry::release_name!(),
                                ..Default::default()
                            },
                        ));
                    }
                },
            }
        };
        let (restart_tx, mut restart_rx) = tokio::sync::mpsc::channel(1);
        let config_tx = ConfigTx { config, restart_tx };

        let handle = tokio::spawn(async move {
            HttpServer::try_from(config_tx)?
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
            _ = async {}, if handle.is_finished() => {
                return match handle.await {
                    Ok(result) => result,
                    Err(e) => Err(ApiError::JoinHandle(e))
                };
            },
        }
    }

    Ok(())
}
