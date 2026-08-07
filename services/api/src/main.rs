#![expect(
    unused_crate_dependencies,
    reason = "dependencies used by lib but not binary"
)]

#[cfg(feature = "sentry")]
use std::path::PathBuf;
use std::sync::Arc;

use bencher_api::api::Api;
use bencher_config::{Config, ConfigTx};
use bencher_json::BENCHER_API_VERSION;
#[cfg(feature = "plus")]
use bencher_json::system::config::JsonLitestream;
use bencher_schema::context::ApiContext;
use dropshot::HttpServer;
use futures_concurrency::future::Race as _;
use futures_util::FutureExt as _;
#[cfg(feature = "sentry")]
use sentry::ClientInitGuard;
use slog::{Logger, error, info};
#[cfg(feature = "plus")]
use tokio::process::Command;
#[cfg(feature = "plus")]
use tokio::sync;
#[cfg(feature = "plus")]
use tokio::task::JoinHandle;
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
        let (restore_tx, restore_rx) = sync::oneshot::channel();
        let litestream_handle = run_litestream(log, &config, litestream, restore_tx)?;
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
        // Signal long-lived handlers (the runner WebSocket channel) to wind down so the in-flight
        // connection drain in `server.close()` can complete instead of hanging until the platform
        // escalates to SIGKILL (which would skip the rate limiting save below entirely).
        ctx.shutdown.cancel();
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

#[cfg(feature = "plus")]
#[derive(Debug, thiserror::Error)]
pub enum LitestreamError {
    #[error("Failed to absolutize the database path: {0}")]
    Database(std::io::Error),
    #[error(
        "Failed to convert Bencher config to Litestream config. This is likely a bug. Please report this: {0}"
    )]
    Yaml(serde_yaml::Error),
    #[error("Failed to write Litestream config ({0}): {1}")]
    WriteYaml(PathBuf, std::io::Error),
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
    RestoreRecv(sync::oneshot::error::RecvError),
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
    restore_tx: sync::oneshot::Sender<()>,
) -> Result<JoinHandle<Result<(), LitestreamError>>, LitestreamError> {
    // Get the absolute database path from the config
    let db_path = if config.database.file.is_absolute() {
        config.database.file.clone()
    } else {
        std::env::current_dir()
            .map_err(LitestreamError::Database)?
            .join(&config.database.file)
    };
    cleanup_stale_litestream_files(log, &db_path);
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
        if !restore.status.success() {
            return Err(LitestreamError::RestoreExit {
                status: restore.status,
                stdout: String::from_utf8_lossy(&restore.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&restore.stderr).into_owned(),
            });
        }

        // Signal the server that restore is complete (DB file exists)
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

// Remove stale Litestream files that current versions no longer read.
// Litestream 0.3 kept its state in a `generations/` directory and a `generation`
// pointer file; Litestream 0.5+ only reads `ltx/`, so 0.3 state left behind by an
// upgrade is dead weight on the data volume. An interrupted `litestream restore`
// can also leave `<db>.tmp*` output files behind. This runs before Litestream is
// spawned, so nothing else has these files open. Failures are logged and ignored:
// cleanup must never block server startup.
#[cfg(feature = "plus")]
fn cleanup_stale_litestream_files(log: &Logger, db_path: &std::path::Path) {
    let Some(db_dir) = db_path.parent() else {
        return;
    };
    let Some(db_file_name) = db_path.file_name().and_then(std::ffi::OsStr::to_str) else {
        return;
    };

    let state_dir = db_dir.join(format!(".{db_file_name}-litestream"));
    let generations_dir = state_dir.join("generations");
    if generations_dir.is_dir() {
        info!(
            log,
            "Removing legacy Litestream 0.3 state: {generations_dir:?}"
        );
        if let Err(e) = std::fs::remove_dir_all(&generations_dir) {
            slog::warn!(
                log,
                "Failed to remove legacy Litestream 0.3 state ({generations_dir:?}): {e}"
            );
        }
    }

    let stale_files = [
        state_dir.join("generation"),
        db_dir.join(format!("{db_file_name}.tmp")),
        db_dir.join(format!("{db_file_name}.tmp-wal")),
        db_dir.join(format!("{db_file_name}.tmp-shm")),
    ];
    for stale_file in stale_files {
        if stale_file.is_file() {
            info!(log, "Removing stale Litestream file: {stale_file:?}");
            if let Err(e) = std::fs::remove_file(&stale_file) {
                slog::warn!(
                    log,
                    "Failed to remove stale Litestream file ({stale_file:?}): {e}"
                );
            }
        }
    }
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

#[cfg(all(test, feature = "plus"))]
mod tests {
    fn discard_logger() -> slog::Logger {
        slog::Logger::root(slog::Discard, slog::o!())
    }

    #[test]
    fn cleanup_stale_litestream_files() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let dir = tmp_dir.path();
        let db_path = dir.join("bencher.db");
        std::fs::write(&db_path, b"db").unwrap();

        // Legacy Litestream 0.3 state
        let state_dir = dir.join(".bencher.db-litestream");
        let generation_dir = state_dir.join("generations/fa74c771d99d6b04/wal");
        std::fs::create_dir_all(&generation_dir).unwrap();
        std::fs::write(generation_dir.join("00000001.wal.lz4"), b"wal").unwrap();
        std::fs::write(state_dir.join("generation"), b"fa74c771d99d6b04").unwrap();

        // Live Litestream 0.5+ state
        let ltx_dir = state_dir.join("ltx/0");
        std::fs::create_dir_all(&ltx_dir).unwrap();
        let ltx_file = ltx_dir.join("0000000000000001-0000000000000001.ltx");
        std::fs::write(&ltx_file, b"ltx").unwrap();

        // Interrupted restore leftovers
        std::fs::write(dir.join("bencher.db.tmp-wal"), b"").unwrap();
        std::fs::write(dir.join("bencher.db.tmp-shm"), b"shm").unwrap();

        super::cleanup_stale_litestream_files(&discard_logger(), &db_path);

        // Stale state is removed
        assert!(!state_dir.join("generations").exists());
        assert!(!state_dir.join("generation").exists());
        assert!(!dir.join("bencher.db.tmp-wal").exists());
        assert!(!dir.join("bencher.db.tmp-shm").exists());
        // The database and live state are untouched
        assert!(db_path.exists());
        assert!(ltx_file.exists());
    }

    #[test]
    fn cleanup_stale_litestream_files_noop() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let db_path = tmp_dir.path().join("bencher.db");

        super::cleanup_stale_litestream_files(&discard_logger(), &db_path);

        assert!(!db_path.exists());
    }
}
