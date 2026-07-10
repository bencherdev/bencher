#![expect(
    unused_crate_dependencies,
    reason = "dependencies used by lib but not binary"
)]

#[cfg(any(feature = "plus", feature = "sentry"))]
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
    #[cfg(feature = "plus")]
    #[error("Replica config: {0}")]
    ReplicaConfig(bencher_replica::ReplicaConfigError),
    #[cfg(feature = "plus")]
    #[error("Replica restore: {0}")]
    ReplicaRestore(bencher_replica::RestoreError),
    #[cfg(feature = "plus")]
    #[error("Replica: {0}")]
    ReplicaSync(bencher_replica::SyncError),
    #[cfg(feature = "plus")]
    #[error("Failed to resolve the database path for replication: {0}")]
    ReplicaDbPath(std::io::Error),
    #[cfg(feature = "plus")]
    #[error("Database path is not valid UTF-8: {0}")]
    ReplicaDbPathUtf8(PathBuf),
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
    let replica_config = config
        .plus
        .as_ref()
        .and_then(|plus| plus.replica.clone())
        .map(bencher_replica::ReplicaConfig::try_from)
        .transpose()
        .map_err(ApiError::ReplicaConfig)?;
    #[cfg(feature = "plus")]
    let replica_shutdown_timeout = replica_config
        .as_ref()
        .map(|replica| replica.shutdown_sync_timeout);

    // Restore precedence: Litestream owns restore whenever it is configured
    // (during the shadow burn-in the replica is NOT the restore source).
    #[cfg(feature = "plus")]
    let litestream_handle = if let Some(litestream) = config
        .plus
        .as_ref()
        .and_then(|plus| plus.litestream.clone())
    {
        let (restore_tx, restore_rx) = sync::oneshot::channel();
        let litestream_handle = run_litestream(log, &config, litestream, restore_tx)?;
        // Wait for Litestream restore to complete (replicate starts in background)
        restore_rx.await.map_err(LitestreamError::RestoreRecv)?;
        Some(litestream_handle)
    } else {
        if let Some(replica) = &replica_config {
            // Same handshake as `litestream restore`: the restore fully
            // completes before any SQLite connection is opened.
            let db_path = replica_db_path(&config.database.file)?;
            let outcome = bencher_replica::restore_if_missing(log, replica, &db_path)
                .await
                .map_err(ApiError::ReplicaRestore)?;
            info!(log, "Replica restore: {outcome:?}");
        }
        None
    };
    // Both configured: the replica runs in shadow mode (no checkpoints;
    // Litestream keeps checkpoint ownership until cutover).
    #[cfg(feature = "plus")]
    let shadow = litestream_handle.is_some();

    let server = create_api_server(config).await?;

    #[cfg(feature = "plus")]
    let mut replica_handle = start_replica(log, &server, replica_config, shadow)?;

    let shutdown_wait = server.wait_for_shutdown();
    #[cfg(feature = "plus")]
    let result = {
        let litestream_wait = async {
            match litestream_handle {
                Some(litestream_handle) => litestream_handle
                    .await
                    .map_err(LitestreamError::JoinHandle)?
                    .map_err(Into::into),
                None => std::future::pending::<Result<(), ApiError>>().await,
            }
        };
        let replica_fatal = async {
            match replica_handle.as_mut() {
                // Resolves only if the replication task dies; storage
                // outages back off internally and never resolve this.
                //
                // SOLE mode only: in shadow mode the replica is the unproven
                // system burning in while Litestream stays authoritative, so
                // a fatal replica error must never take down a healthy
                // production server. The task still logs and meters the
                // fatal; it just does not race the server here.
                Some(replica_handle) if !shadow => replica_handle
                    .wait_fatal()
                    .await
                    .map_err(ApiError::ReplicaSync),
                Some(_) | None => std::future::pending::<Result<(), ApiError>>().await,
            }
        };
        (
            tokio::signal::ctrl_c().map(|r| r.map_err(ApiError::CtrlC)),
            litestream_wait,
            replica_fatal,
            shutdown_wait.map(|r| r.map_err(ApiError::RunServer)),
        )
            .race()
            .await
    };
    #[cfg(not(feature = "plus"))]
    let result = (
        tokio::signal::ctrl_c().map(|r| r.map_err(ApiError::CtrlC)),
        shutdown_wait.map(|r| r.map_err(ApiError::RunServer)),
    )
        .race()
        .await;

    shutdown(log, server).await;

    // Final replica sync AFTER the server has drained: ship the remaining WAL
    // tail within the shutdown budget. On a COMPLETE drain in sole mode a
    // final checkpoint then runs (after the budget, unbounded) to seal the
    // epoch so the next boot resumes in place; on the deadline the tail stays
    // in the local WAL (lag, never loss).
    #[cfg(feature = "plus")]
    if let Some(replica_handle) = replica_handle {
        // `replica_shutdown_timeout` is always `Some` here (it is derived from
        // the same `replica_config` that produced this handle); the fallback
        // is defensive and reuses the crate default rather than a magic value.
        let deadline =
            replica_shutdown_timeout.unwrap_or(bencher_replica::DEFAULT_SHUTDOWN_SYNC_TIMEOUT);
        if let Err(e) = replica_handle.shutdown(deadline).await {
            error!(log, "Replica final sync failed: {e}");
        }
    }

    result
}

/// Spawn the in-process replication task over the server's database.
#[cfg(feature = "plus")]
fn start_replica(
    log: &Logger,
    server: &HttpServer<ApiContext>,
    replica_config: Option<bencher_replica::ReplicaConfig>,
    shadow: bool,
) -> Result<Option<bencher_replica::ReplicatorHandle>, ApiError> {
    let Some(replica) = replica_config else {
        return Ok(None);
    };
    let ctx = server.app_private();
    let db_path = replica_db_path(&ctx.database.path)?;
    Ok(Some(bencher_replica::Replicator::start(
        log.clone(),
        replica,
        bencher_replica::ReplicaDb {
            db_path,
            writer: ctx.database.connection.clone(),
            busy_timeout_ms: ctx.database.busy_timeout,
        },
        ctx.clock.clone(),
        shadow,
    )))
}

/// The absolute, UTF-8 database path handed to the replicator.
///
/// Canonicalized when the file exists: `SQLite` resolves a symlinked
/// database file and creates the real `-wal` next to the TARGET, so the
/// replicator must derive its WAL path from the resolved location or it
/// would silently watch a never-existing file.
#[cfg(feature = "plus")]
fn replica_db_path(file: &std::path::Path) -> Result<camino::Utf8PathBuf, ApiError> {
    let absolute = if file.is_absolute() {
        file.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(ApiError::ReplicaDbPath)?
            .join(file)
    };
    let resolved = match std::fs::canonicalize(&absolute) {
        Ok(resolved) => resolved,
        // A missing database (fresh start before the first connection) has
        // nothing to resolve yet; canonicalize the parent instead so a
        // symlinked data directory still lands on the real location.
        Err(_missing) => match (absolute.parent(), absolute.file_name()) {
            (Some(parent), Some(file_name)) => std::fs::canonicalize(parent)
                .map(|parent| parent.join(file_name))
                .unwrap_or(absolute),
            _ => absolute,
        },
    };
    camino::Utf8PathBuf::from_path_buf(resolved).map_err(ApiError::ReplicaDbPathUtf8)
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

async fn create_api_server(config: Config) -> Result<HttpServer<ApiContext>, ApiError> {
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ServerStartup);

    let config_tx = ConfigTx { config };
    config_tx
        .into_server::<Api>()
        .await
        .map_err(ApiError::ConfigTxError)
}
