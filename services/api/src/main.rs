#![expect(
    unused_crate_dependencies,
    reason = "dependencies used by lib but not binary"
)]

#[cfg(feature = "sentry")]
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(feature = "plus")]
use std::time::Duration;

use bencher_api::api::Api;
use bencher_config::{Config, ConfigTx};
use bencher_json::BENCHER_API_VERSION;
#[cfg(feature = "plus")]
use bencher_json::system::config::JsonDisasterRecovery;
use bencher_schema::context::ApiContext;
use dropshot::HttpServer;
use futures_concurrency::future::Race as _;
use futures_util::FutureExt as _;
#[cfg(all(feature = "plus", unix))]
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
#[cfg(feature = "sentry")]
use sentry::ClientInitGuard;
use slog::{Logger, error, info};
#[cfg(all(feature = "plus", unix))]
use tokio::process::Child;
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};
#[cfg(feature = "plus")]
use tokio::{process::Command, sync::oneshot, task::JoinSet};
use tokio_rustls::rustls::crypto::{CryptoProvider, aws_lc_rs};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Failed to build the async runtime: {0}")]
    Runtime(std::io::Error),
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
    #[cfg(unix)]
    #[error("Failed to listen for the SIGTERM signal: {0}")]
    Sigterm(std::io::Error),
    #[cfg(unix)]
    #[error("Failed to listen for the SIGHUP signal: {0}")]
    Sighup(std::io::Error),
    #[error("Shutting down server: {0}")]
    RunServer(String),
}

fn main() -> Result<(), ApiError> {
    bencher_api::runtime()
        .map_err(ApiError::Runtime)?
        .block_on(serve())
}

async fn serve() -> Result<(), ApiError> {
    let log = bencher_logger::bootstrap_logger();
    #[cfg(feature = "sentry")]
    let guard = sentry::init(sentry_options());
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
        if let Some(hint) = e.hint() {
            error!(&log, "{hint}");
        }
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
    if let Some(disaster_recovery) = config
        .plus
        .as_ref()
        .and_then(|plus| plus.disaster_recovery.clone())
    {
        let litestream = Litestream::new(&config, disaster_recovery)?;
        litestream.restore(log).await?;
        // The server replicates once its checks pass, a license among them on Bencher
        // Self-Hosted, and before it serves.
        let (server, mut replicator) =
            Box::pin(create_api_server_with(config, || litestream.replicate())).await?;
        let result = serve_until_stopped(log, server, async {
            replicator.stopped().await.map_err(Into::into)
        })
        .await;
        // After the server closes, so Litestream replicates its last writes.
        replicator.stop(log).await;

        return result;
    }

    let server = create_api_server(config).await?;
    serve_until_stopped(log, server, std::future::pending()).await
}

/// Serve until the stop signal, the server stopping, or `until`, then shut the server down.
async fn serve_until_stopped<F>(
    log: &Logger,
    server: HttpServer<ApiContext>,
    until: F,
) -> Result<(), ApiError>
where
    F: Future<Output = Result<(), ApiError>>,
{
    let shutdown_wait = server.wait_for_shutdown();
    let result = (
        stop_signal(),
        until,
        shutdown_wait.map(|r| r.map_err(ApiError::RunServer)),
    )
        .race()
        .await;

    shutdown(log, server).await;

    result
}

/// The signal that stops the server: ctrl-c, the SIGTERM with which a host stops a service, or the
/// SIGHUP of a terminal that hangs up.
async fn stop_signal() -> Result<(), ApiError> {
    (
        tokio::signal::ctrl_c().map(|r| r.map_err(ApiError::CtrlC)),
        terminated(),
    )
        .race()
        .await
}

/// The first SIGTERM, or the first SIGHUP of a terminal that hangs up. Litestream runs in its
/// own process group, so a hangup reaches the API alone, and the API stops Litestream.
#[cfg(unix)]
async fn terminated() -> Result<(), ApiError> {
    let mut terminate = signal(SignalKind::terminate()).map_err(ApiError::Sigterm)?;
    let mut hangup = signal(SignalKind::hangup()).map_err(ApiError::Sighup)?;
    (terminate.recv(), hangup.recv()).race().await;
    Ok(())
}

/// Only Unix has SIGTERM and SIGHUP.
#[cfg(not(unix))]
async fn terminated() -> Result<(), ApiError> {
    std::future::pending().await
}

async fn shutdown(log: &Logger, server: HttpServer<ApiContext>) {
    // Everything shutdown needs out of the context, taken before `server.close()` consumes the
    // server.
    #[cfg(feature = "plus")]
    let (rate_limiting, database_path, prune_task, callbacks, upload_sweep) = {
        let ctx = server.app_private();
        // Signal long-lived handlers (the runner WebSocket channel), the periodic rate limiting
        // prune, and callback deliveries to wind down, so the in-flight connection drain in
        // `server.close()` can complete instead of hanging until the platform escalates to SIGKILL
        // (which would skip the rate limiting save below entirely).
        ctx.shutdown.cancel();
        (
            ctx.rate_limiting.clone(),
            ctx.database.path.clone(),
            ctx.rate_limiting_prune.take(),
            ctx.callbacks.clone(),
            ctx.oci_storage.close_upload_sweep(),
        )
    };
    if let Err(e) = server.close().await {
        error!(log, "Server close error: {e}");
    }
    #[cfg(feature = "plus")]
    {
        // Let the periodic task finish any save already in flight, so it cannot race the final one
        // over the shared partial file. It stops promptly: the shutdown signal above ends its wait
        // for the next tick, and a save in progress is a few milliseconds of synchronous work.
        if let Some(prune_task) = prune_task
            && let Err(e) = prune_task.await
        {
            error!(log, "Rate limiting prune task failed to join: {e}");
        }
        rate_limiting.prune_and_save(&database_path, log);
        upload_sweep.stop(log).await;
        // Last, so a delivery slow to stop never delays the save. No handler is left to fire a
        // callback, and a detached heartbeat timeout that fires one after the drain begins finds
        // the owner closed and leaves it pending for the next start.
        callbacks.drain(log).await;
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
            sentry::init((sentry_dsn.as_ref(), sentry_options()))
        })
}

#[cfg(feature = "sentry")]
fn sentry_options() -> sentry::ClientOptions {
    let mut options = sentry::ClientOptions::new();
    options.release = sentry::release_name!();
    options
}

#[cfg(feature = "plus")]
#[derive(Debug, thiserror::Error)]
pub enum LitestreamError {
    #[error("Failed to absolutize the database path: {0}")]
    Database(std::io::Error),
    #[error(
        "Failed to convert Bencher config to Litestream config. This is likely a bug. Please report this: {0}"
    )]
    Serialize(serde_json::Error),
    #[error("Failed to write Litestream config ({0}): {1}")]
    WriteConfig(PathBuf, std::io::Error),
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
    #[error("Failed to replicate: {0}")]
    ReplicateExit(std::process::ExitStatus),
    #[error("Failed to join Litestream handle: {0}")]
    JoinHandle(tokio::task::JoinError),
    #[cfg(unix)]
    #[error("Litestream's process ID ({0}) does not fit a signal")]
    ProcessId(u32),
    #[cfg(unix)]
    #[error("Failed to send Litestream SIGTERM: {0}")]
    Terminate(nix::errno::Errno),
}

/// Litestream, configured to restore and replicate the server's database.
#[cfg(feature = "plus")]
struct Litestream {
    config_path: PathBuf,
    db_path: PathBuf,
}

#[cfg(feature = "plus")]
impl Litestream {
    /// Write the Litestream config for the server's database.
    fn new(
        config: &Config,
        disaster_recovery: JsonDisasterRecovery,
    ) -> Result<Self, LitestreamError> {
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
        let litestream_config = disaster_recovery
            .into_litestream_config(db_path.clone(), config.logging.log.level())
            .map_err(LitestreamError::Serialize)?;
        std::fs::write(&config_path, litestream_config)
            .map_err(|e| LitestreamError::WriteConfig(config_path.clone(), e))?;
        Ok(Self {
            config_path,
            db_path,
        })
    }

    /// Restore the database from its replica, when the replica exists and the database does not.
    async fn restore(&self, log: &Logger) -> Result<(), LitestreamError> {
        // https://litestream.io/reference/restore/
        let restore = Command::new("litestream")
            .arg("restore")
            .arg("-if-replica-exists")
            .arg("-if-db-not-exists")
            .arg("-config")
            .arg(&self.config_path)
            .arg("-no-expand-env")
            .arg(&self.db_path)
            .output()
            .await
            .map_err(LitestreamError::Restore)?;
        slog::info!(log, "Litestream restore: {restore:?}");
        if restore.status.success() {
            Ok(())
        } else {
            Err(LitestreamError::RestoreExit {
                status: restore.status,
                stdout: String::from_utf8_lossy(&restore.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&restore.stderr).into_owned(),
            })
        }
    }

    /// Replicate the database, which runs until the server stops.
    fn replicate(&self) -> Replicator {
        // https://litestream.io/reference/replicate/
        let mut replicate = Command::new("litestream");
        replicate
            .arg("replicate")
            .arg("-config")
            .arg(&self.config_path)
            .arg("-no-expand-env");
        Replicator::start(replicate)
    }
}

/// How long a normal stop waits for Litestream to exit after SIGTERM, its last sync included,
/// before the drop kills it.
#[cfg(feature = "plus")]
const LITESTREAM_STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// A running replicator. A normal stop sends it SIGTERM and waits for it to exit, so Litestream
/// finishes its last sync. A drop, as on a start that fails after replication began, or a stop
/// past [`LITESTREAM_STOP_TIMEOUT`], aborts the task, and the process, killed on drop, ends with
/// it, so none is left behind.
#[cfg(feature = "plus")]
struct Replicator {
    // Before `stop`, so a drop aborts the task before the task can see its channel close.
    tasks: JoinSet<Result<(), LitestreamError>>,
    stop: oneshot::Sender<()>,
}

#[cfg(feature = "plus")]
impl Replicator {
    /// Start `command` now, as the replicator.
    fn start(mut command: Command) -> Self {
        // Its own process group, so a terminal's ctrl-c reaches the API alone, and the only stop
        // signal Litestream gets is the one that `terminate` sends.
        #[cfg(unix)]
        command.process_group(0);
        let child = command.kill_on_drop(true).spawn();
        let (stop, stopping) = oneshot::channel();
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            let mut child = child.map_err(LitestreamError::Replicate)?;
            // Litestream runs until the server stops it, so any exit before that is an error.
            if let Some(exited) = (child.wait().map(Some), stopping.map(|_stop| None))
                .race()
                .await
            {
                return Err(LitestreamError::ReplicateExit(
                    exited.map_err(LitestreamError::Replicate)?,
                ));
            }
            // Without SIGTERM, the stop's bound passes and the drop kills Litestream.
            #[cfg(unix)]
            terminate(&child)?;
            child.wait().await.map_err(LitestreamError::Replicate)?;
            Ok(())
        });
        Self { tasks, stop }
    }

    /// Wait for the replicator to stop, which it does on its own only on an error.
    async fn stopped(&mut self) -> Result<(), LitestreamError> {
        Self::joined(&mut self.tasks).await
    }

    /// Stop the replicator once the server closes: SIGTERM, then a wait of up to
    /// [`LITESTREAM_STOP_TIMEOUT`] for Litestream to finish its last sync and exit. Past that
    /// bound, the drop kills it.
    async fn stop(self, log: &Logger) {
        let Self { mut tasks, stop } = self;
        // A replicator that no longer listens has already stopped.
        if stop.send(()).is_err() {
            return;
        }
        match tokio::time::timeout(LITESTREAM_STOP_TIMEOUT, Self::joined(&mut tasks)).await {
            Ok(Ok(())) => info!(log, "Litestream stopped"),
            Ok(Err(e)) => error!(log, "Litestream failed to stop: {e}"),
            Err(_elapsed) => error!(
                log,
                "Litestream did not stop within {LITESTREAM_STOP_TIMEOUT:?} of SIGTERM, so it is killed"
            ),
        }
    }

    async fn joined(
        tasks: &mut JoinSet<Result<(), LitestreamError>>,
    ) -> Result<(), LitestreamError> {
        while let Some(joined) = tasks.join_next().await {
            joined.map_err(LitestreamError::JoinHandle)??;
        }
        Ok(())
    }
}

/// Ask Litestream to stop with SIGTERM, so it finishes its last sync.
///
/// This is Litestream's only stop signal. Litestream skips or cancels its shutdown sync on a
/// second SIGINT or SIGTERM, so the API is the only process that stops it: Litestream runs in
/// its own process group, and a host must send its stop signal to the API alone.
#[cfg(all(feature = "plus", unix))]
fn terminate(child: &Child) -> Result<(), LitestreamError> {
    // A child without an ID has already exited.
    let Some(id) = child.id() else {
        return Ok(());
    };
    let pid = i32::try_from(id).map_err(|_overflow| LitestreamError::ProcessId(id))?;
    kill(Pid::from_raw(pid), Signal::SIGTERM).map_err(LitestreamError::Terminate)
}

async fn create_api_server(config: Config) -> Result<HttpServer<ApiContext>, ApiError> {
    create_api_server_with(config, || ())
        .await
        .map(|(server, ())| server)
}

/// The API server, with `before_serve` run once its checks pass and before it serves.
async fn create_api_server_with<B, T>(
    config: Config,
    before_serve: B,
) -> Result<(HttpServer<ApiContext>, T), ApiError>
where
    B: FnOnce() -> T,
{
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ServerStartup);

    let config_tx = ConfigTx { config };
    config_tx
        .into_server_with::<Api, _, _>(before_serve)
        .await
        .map_err(ApiError::ConfigTxError)
}

impl ApiError {
    /// What to try when the server fails to start for this error.
    fn hint(&self) -> Option<&'static str> {
        if let Self::ConfigTxError(error) = self
            && error.is_bind_error()
        {
            Some("Is another instance running on the same port?")
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    #[cfg(all(feature = "plus", unix))]
    use std::path::Path;

    #[cfg(all(feature = "plus", unix))]
    use nix::sys::signal::Signal;
    #[cfg(all(feature = "plus", unix))]
    use tokio::{
        io::AsyncReadExt as _,
        net::unix::pipe::Receiver,
        signal::unix::{SignalKind, signal},
    };

    use super::ApiError;
    #[cfg(all(feature = "plus", unix))]
    use super::{LITESTREAM_STOP_TIMEOUT, Replicator};

    /// A stand-in replica that runs `script` in `sh` with `marker` as `$1`, and the read end of
    /// its output. The replica holds the only write end, so a read ends once the replica does.
    #[cfg(all(feature = "plus", unix))]
    fn stand_in(script: &str, marker: &Path) -> (Replicator, Receiver) {
        let (out, replica_out) = std::io::pipe().unwrap();
        let mut replica = tokio::process::Command::new("sh");
        replica
            .arg("-c")
            .arg(script)
            .arg("sh")
            .arg(marker)
            .stdout(replica_out);
        (
            Replicator::start(replica),
            Receiver::from_owned_fd(out.into()).unwrap(),
        )
    }

    /// Wait for the stand-in replica to say that its signal handling is in place.
    #[cfg(all(feature = "plus", unix))]
    async fn ready(out: &mut Receiver) {
        let mut line = [0; 6];
        out.read_exact(&mut line).await.unwrap();
        assert_eq!(&line, b"ready\n");
    }

    /// Wait for the stand-in replica's process to end, bounded so a replica that outlives its
    /// owner fails rather than hangs.
    #[cfg(all(feature = "plus", unix))]
    async fn ended(out: &mut Receiver) {
        let mut rest = Vec::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            out.read_to_end(&mut rest),
        )
        .await
        .expect("the replica outlived its owner")
        .unwrap();
    }

    #[cfg(all(feature = "plus", unix))]
    fn discard() -> slog::Logger {
        slog::Logger::root(slog::Discard, slog::o!())
    }

    /// Start a server without disaster recovery, send this process `sent`, which the stop signal
    /// listens for as `kind`, and check that the server shuts down through `shutdown`.
    #[cfg(all(feature = "plus", unix))]
    async fn shuts_down_on(kind: SignalKind, sent: Signal) {
        use futures_util::FutureExt as _;
        use nix::{sys::signal::kill, unistd::Pid};

        let dir = tempfile::tempdir().unwrap();
        let mut json = bencher_config::Config::default().into_inner();
        json.server.bind_address = "127.0.0.1:0".parse().unwrap();
        json.database.file = dir.path().join("bencher.db");
        let server = super::create_api_server(json.into()).await.unwrap();
        let address = server.local_addr();
        let log = discard();
        // The test listens too, so a signal that the server misses fails the test rather than
        // ending its process.
        let _listening = signal(kind).unwrap();
        let mut serving = std::pin::pin!(super::serve_until_stopped(
            &log,
            server,
            std::future::pending()
        ));
        // The first poll puts the server's signal handlers in place.
        assert!(serving.as_mut().now_or_never().is_none());

        kill(Pid::this(), sent).unwrap();

        // A bound on the wait, so a server that misses the signal fails rather than hangs.
        tokio::time::timeout(std::time::Duration::from_secs(10), serving)
            .await
            .unwrap_or_else(|_elapsed| panic!("{sent} did not shut the server down"))
            .unwrap();
        // `shutdown` closed the server, so nothing accepts on its port.
        std::net::TcpStream::connect(address).unwrap_err();
    }

    // Fails if a server without disaster recovery does not shut down through `shutdown` on the
    // SIGTERM with which a host stops a service, as one that races only ctrl-c does not.
    #[cfg(all(feature = "plus", unix))]
    #[tokio::test]
    async fn sigterm_shuts_down_a_server_without_disaster_recovery() {
        shuts_down_on(SignalKind::terminate(), Signal::SIGTERM).await;
    }

    // Fails if a server does not shut down through `shutdown` on the SIGHUP of a terminal that
    // hangs up, which would end the API at once and leave Litestream, in its own process group,
    // running.
    #[cfg(all(feature = "plus", unix))]
    #[tokio::test]
    async fn sighup_shuts_down_a_server_without_disaster_recovery() {
        shuts_down_on(SignalKind::hangup(), Signal::SIGHUP).await;
    }

    // Fails if a normal stop kills the replica, which cuts Litestream's last sync, rather than
    // sending it SIGTERM and waiting for it to exit.
    #[cfg(all(feature = "plus", unix))]
    #[tokio::test]
    async fn a_normal_stop_sends_sigterm_and_waits_for_the_replica() {
        let dir = tempfile::tempdir().unwrap();
        let synced = dir.path().join("synced");
        // On SIGTERM the replica takes a moment to write its marker, as Litestream's last sync
        // does, then exits.
        let (replicator, mut out) = stand_in(
            r#"trap 'sleep 0.2; touch "$1"; exit 0' TERM; echo ready; while :; do sleep 0.05; done"#,
            &synced,
        );
        ready(&mut out).await;

        replicator.stop(&discard()).await;

        assert!(synced.exists(), "the replica did not finish its last sync");
        ended(&mut out).await;
    }

    // Fails if the replica shares the API's process group, where a terminal's ctrl-c reaches it
    // too: the API's SIGTERM is then Litestream's second stop signal, which skips its last sync.
    #[cfg(all(feature = "plus", unix))]
    #[tokio::test]
    async fn the_replica_has_its_own_process_group() {
        use nix::unistd::{Pid, getpgid, getpgrp};
        use tokio::io::{AsyncBufReadExt as _, BufReader};

        let dir = tempfile::tempdir().unwrap();
        let (replicator, mut out) = stand_in("echo $$; exec sleep 600", &dir.path().join("unused"));
        let mut id = String::new();
        BufReader::new(&mut out).read_line(&mut id).await.unwrap();
        let replica = Pid::from_raw(id.trim().parse().unwrap());

        let group = getpgid(Some(replica)).unwrap();

        assert_eq!(
            group, replica,
            "the replica does not lead its own process group"
        );
        assert_ne!(
            group,
            getpgrp(),
            "the replica is in the API's process group"
        );
        drop(replicator);
        ended(&mut out).await;
    }

    // Fails if a replica that ignores SIGTERM holds a normal stop past its bound, the stop gives
    // up before the bound, or the replica outlives the stop.
    #[cfg(all(feature = "plus", unix))]
    #[tokio::test]
    async fn a_replica_that_ignores_sigterm_is_killed_past_the_bound() {
        let dir = tempfile::tempdir().unwrap();
        let (replicator, mut out) = stand_in(
            "trap '' TERM; echo ready; exec sleep 600",
            &dir.path().join("unused"),
        );
        ready(&mut out).await;

        tokio::time::pause();
        let started = tokio::time::Instant::now();
        replicator.stop(&discard()).await;
        let waited = started.elapsed();
        tokio::time::resume();

        // Time is paused, so the wait is exactly the bound, and a longer bound shows.
        assert!(
            (LITESTREAM_STOP_TIMEOUT..LITESTREAM_STOP_TIMEOUT * 2).contains(&waited),
            "the stop waited {waited:?}"
        );
        ended(&mut out).await;
    }

    // Fails if a start that fails after replication began leaves the replica running, as a
    // replicator that its owner detaches on drop does.
    #[cfg(all(feature = "plus", unix))]
    #[tokio::test]
    async fn a_failed_start_leaves_no_replica_running() {
        let dir = tempfile::tempdir().unwrap();
        let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut json = bencher_config::Config::default().into_inner();
        json.server.bind_address = held.local_addr().unwrap();
        json.database.file = dir.path().join("bencher.db");
        // The replica holds the only write end of the pipe, so the read ends once the replica
        // does.
        let (replica_out, replica_in) = std::io::pipe().unwrap();
        let mut replica_out = Receiver::from_owned_fd(replica_out.into()).unwrap();
        let mut replica = tokio::process::Command::new("sleep");
        replica.arg("600").stdout(replica_in);

        let started =
            super::create_api_server_with(json.into(), || Replicator::start(replica)).await;

        assert!(
            matches!(&started, Err(ApiError::ConfigTxError(error)) if error.is_bind_error()),
            "the start did not fail to bind"
        );
        drop(started);
        ended(&mut replica_out).await;
    }

    // Fails if a server that could not bind its port does not ask whether another instance holds
    // it, or one refused for any other reason, such as the license that disaster recovery needs,
    // does.
    #[test]
    fn only_a_bind_failure_hints_at_another_instance() {
        let address: SocketAddr = "127.0.0.1:6610".parse().unwrap();
        let bind = ApiError::ConfigTxError(bencher_config::ConfigTxError::CreateServer(Box::new(
            dropshot::BuildError::BindError {
                address,
                error: std::io::Error::from(std::io::ErrorKind::AddrInUse),
            },
        )));
        assert!(bind.hint().is_some_and(|hint| hint.contains("same port")));

        #[cfg(feature = "plus")]
        assert_eq!(
            ApiError::ConfigTxError(bencher_config::ConfigTxError::DisasterRecoveryLicense).hint(),
            None
        );
        assert_eq!(
            ApiError::CtrlC(std::io::Error::from(std::io::ErrorKind::Interrupted)).hint(),
            None
        );
    }
}
