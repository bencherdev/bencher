#![expect(
    unused_crate_dependencies,
    reason = "dependencies used by lib but not binary"
)]

use std::sync::Arc;

use bencher_api::api::Api;
use bencher_config::{Config, ConfigTx};
use bencher_json::BENCHER_API_VERSION;
#[cfg(feature = "plus")]
use bencher_litestream::{LitestreamError, LitestreamLevel, run_litestream};
use bencher_schema::context::ApiContext;
#[cfg(feature = "plus")]
use camino::Utf8PathBuf;
use dropshot::HttpServer;
use futures_concurrency::future::Race as _;
use futures_util::FutureExt as _;
#[cfg(feature = "sentry")]
use sentry::ClientInitGuard;
use slog::{Logger, error, info};
#[cfg(feature = "plus")]
use tokio::sync;
use tokio_rustls::rustls::crypto::{CryptoProvider, aws_lc_rs};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Failed to install default TLS crypto provider: {0:?}")]
    Rustls(Arc<CryptoProvider>),
    #[error("{0}")]
    Config(bencher_config::ConfigError),
    #[cfg(all(feature = "plus", feature = "otel"))]
    #[error("Failed to initialize OpenTelemetry: {0}")]
    OpenTelemetry(bencher_otel_provider::OtelProviderError),
    #[cfg(feature = "plus")]
    #[error("{0}")]
    Litestream(#[from] LitestreamError),
    #[error("{0}")]
    ConfigTxError(bencher_config::ConfigTxError),
    #[error("Failed to listen for ctrl-c signal: {0}")]
    CtrlC(std::io::Error),
    #[error("Shutting down server: {0}")]
    RunServer(String),
}

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    let log = bencher_logger::bootstrap_logger();
    #[cfg(feature = "sentry")]
    let guard = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    });
    info!(&log, "🐰 Bencher API Server v{BENCHER_API_VERSION}");

    let crypto_provider = aws_lc_rs::default_provider();
    crypto_provider
        .install_default()
        .map_err(ApiError::Rustls)
        .inspect_err(|e| error!(&log, "{e}"))?;

    if let Err(e) = run(
        &log,
        #[cfg(feature = "sentry")]
        guard,
    )
    .await
    {
        error!(&log, "Server failed to run: {e}");
        error!(&log, "Is another instance running on the same port?");
        return Err(e);
    }
    Ok(())
}

async fn run(
    log: &Logger,
    #[cfg(feature = "sentry")] mut _guard: ClientInitGuard,
) -> Result<(), ApiError> {
    let config = Config::load_or_default(log)
        .await
        .map_err(ApiError::Config)?;

    #[cfg(all(feature = "plus", feature = "sentry"))]
    let _guard = init_sentry(log, &config);

    #[cfg(all(feature = "plus", feature = "otel"))]
    let _otel_guard = bencher_otel_provider::run_open_telemetry(log, &config)
        .inspect_err(|e| {
            error!(log, "Failed to run OpenTelemetry: {e}");
            #[cfg(feature = "sentry")]
            sentry::capture_error(&e);
        })
        .map_err(ApiError::OpenTelemetry)?;

    #[cfg(feature = "plus")]
    if let Some(litestream) = config
        .plus
        .as_ref()
        .and_then(|plus| plus.litestream.clone())
    {
        // Absolutize the database path so Litestream resolves it independently of the CWD.
        let db_path = if config.database.file.is_absolute() {
            config.database.file.clone()
        } else {
            std::env::current_dir()
                .map_err(LitestreamError::Database)?
                .join(&config.database.file)
        };
        let db_path =
            Utf8PathBuf::from_path_buf(db_path).map_err(LitestreamError::DatabasePathNotUtf8)?;
        #[cfg(debug_assertions)]
        let config_path = Utf8PathBuf::from("etc/litestream.yml");
        #[cfg(not(debug_assertions))]
        let config_path = Utf8PathBuf::from("/etc/litestream.yml");
        let log_level = LitestreamLevel::from(config.logging.log.level());

        let (restore_tx, restore_rx) = sync::oneshot::channel();
        let litestream_handle =
            run_litestream(log, litestream, db_path, config_path, log_level, restore_tx)?;
        // Wait for Litestream restore to complete (replicate starts in background)
        restore_rx.await.map_err(LitestreamError::RestoreRecv)?;

        let server = create_api_server(config).await?;
        let shutdown_wait = server.wait_for_shutdown();
        let result = (
            tokio::signal::ctrl_c().map(|r| r.map_err(ApiError::CtrlC)),
            async {
                litestream_handle
                    .await
                    .map_err(LitestreamError::JoinHandle)?
                    .map_err(Into::into)
            },
            shutdown_wait.map(|r| r.map_err(ApiError::RunServer)),
        )
            .race()
            .await;

        shutdown(log, server).await;

        return result;
    }

    let server = create_api_server(config).await?;
    let shutdown_wait = server.wait_for_shutdown();
    let result = (
        tokio::signal::ctrl_c().map(|r| r.map_err(ApiError::CtrlC)),
        shutdown_wait.map(|r| r.map_err(ApiError::RunServer)),
    )
        .race()
        .await;

    shutdown(log, server).await;

    result
}

async fn shutdown(log: &Logger, server: HttpServer<ApiContext>) {
    #[cfg(feature = "plus")]
    let save_rate_limiting = {
        let ctx = server.app_private();
        let rate_limiting = ctx.rate_limiting.clone();
        let database_path = ctx.database.path.clone();
        rate_limiting.prune();
        move || rate_limiting.save(&database_path, log)
    };
    if let Err(e) = server.close().await {
        error!(log, "Server close error: {e}");
    }
    #[cfg(feature = "plus")]
    if let Err(e) = save_rate_limiting() {
        error!(log, "Failed to save rate limiting state: {e}");
    }
}

#[cfg(all(feature = "plus", feature = "sentry"))]
fn init_sentry(log: &Logger, config: &Config) -> Option<ClientInitGuard> {
    config
        .plus
        .as_ref()
        .and_then(|plus| plus.cloud.as_ref())
        .and_then(|cloud| cloud.sentry.as_ref())
        .map(|sentry_dsn| {
            info!(log, "Initializing Sentry for error tracking");
            sentry::init((
                sentry_dsn.as_ref(),
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    ..Default::default()
                },
            ))
        })
}

async fn create_api_server(config: Config) -> Result<HttpServer<ApiContext>, ApiError> {
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ServerStartup);

    let config_tx = ConfigTx { config };
    config_tx
        .into_server::<Api>()
        .await
        .map_err(ApiError::ConfigTxError)
}
