use std::{
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
};

#[cfg(feature = "plus")]
use bencher_callback::{CallbackClient, CallbackKey, error_chain};
use bencher_endpoint::Registrar;
#[cfg(feature = "plus")]
use bencher_json::system::config::{
    DEFAULT_HEARTBEAT_TIMEOUT_SECS, DEFAULT_JOB_TIMEOUT_GRACE_PERIOD_SECS, JsonPlus,
};
use bencher_json::{
    JsonConfig,
    system::config::{
        IfExists, JsonConsole, JsonDatabase, JsonLogging, JsonSecurity, JsonServer, JsonSmtp,
        JsonTls, LogLevel, ServerLog,
    },
};
use bencher_rbac::init_rbac;
use bencher_schema::context::{ApiContext, Database, DbConnection};
#[cfg(feature = "plus")]
use bencher_schema::{
    context::{Callbacks, RateLimiting},
    model::{
        organization::license::any_valid_license,
        runner::{JobTimeout, job::reprocess_completed_jobs},
        server::{DailyStats, QueryServer},
    },
    write_conn,
};
use bencher_token::TokenKey;
use diesel::{
    Connection as _,
    connection::SimpleConnection as _,
    r2d2::{ConnectionManager, Pool},
};
use dropshot::{
    ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingIfExists, ConfigLoggingLevel,
    ConfigTls, HttpServer,
};
use slog::{Logger, debug, error, info};

#[cfg(feature = "plus")]
use super::plus::Plus;
use super::{Config, DEFAULT_BUSY_TIMEOUT, DEFAULT_CACHE_SIZE};

const DATABASE_URL: &str = "DATABASE_URL";
const SQLITE_TMPDIR: &str = "SQLITE_TMPDIR";

pub struct ConfigTx {
    pub config: Config,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigTxError {
    #[error("Failed to create server logger: {0}")]
    CreateLogger(std::io::Error),
    #[error("{0}")]
    Migrations(#[from] bencher_schema::MigrationError),
    #[error("Failed to run database pragma: {0}")]
    Pragma(diesel::result::Error),
    #[error("Failed to create temp directory ({0}): {1}")]
    TempDir(PathBuf, std::io::Error),
    #[error("Failed to parse role based access control (RBAC) rules: {0}")]
    Polar(Box<oso::OsoError>),
    #[error("Invalid endpoint URL: {0}")]
    Endpoint(bencher_json::ValidError),
    #[error("Failed to connect to database ({0}): {1}")]
    DatabaseConnection(String, diesel::ConnectionError),
    #[error("Failed to create database connection pool ({0}): {1}")]
    DatabaseConnectionPool(String, diesel::r2d2::PoolError),
    #[error("Failed to register endpoint: {0}")]
    Register(dropshot::ApiDescriptionRegisterError),
    #[error("Failed to create server: {0}")]
    CreateServer(Box<dyn std::error::Error + Send + Sync>),

    #[cfg(feature = "plus")]
    #[error("{0}")]
    Plus(super::plus::PlusError),
    #[cfg(feature = "plus")]
    #[error("Failed to get server ID: {0}")]
    ServerId(dropshot::HttpError),
    #[cfg(feature = "plus")]
    #[error("Invalid runner update base URL: {0}")]
    RunnerUpdateBaseUrl(bencher_json::ValidError),
    #[cfg(feature = "plus")]
    #[error("Failed to spawn stats: {0}")]
    SpawnStats(dropshot::HttpError),
    #[cfg(feature = "plus")]
    #[error("{0}")]
    CallbackKey(bencher_callback::CallbackKeyError),
    #[cfg(feature = "plus")]
    #[error("{0}")]
    OAuthSigningKey(bencher_token::OAuthSigningKeyError),
    #[cfg(feature = "plus")]
    #[error(
        "Disaster recovery (`plus.disaster_recovery`) needs a valid Bencher Plus Enterprise license on at least one organization on this server, and none was found. To add one, remove `plus.disaster_recovery` from the server configuration, start the server, add the license to an organization, then restore `plus.disaster_recovery` and restart: https://bencher.dev/pricing"
    )]
    DisasterRecoveryLicense,
    #[cfg(feature = "plus")]
    #[error("Failed to check the licenses for disaster recovery (`plus.disaster_recovery`): {0}")]
    DisasterRecoveryCheck(dropshot::HttpError),
}

impl ConfigTxError {
    /// Whether the server failed to bind its address, as when another server holds its port.
    pub fn is_bind_error(&self) -> bool {
        if let Self::CreateServer(error) = self {
            matches!(
                error.downcast_ref::<dropshot::BuildError>(),
                Some(dropshot::BuildError::BindError { .. })
            )
        } else {
            false
        }
    }
}

/// Makes Bencher Plus from its configuration, for the console URL and the database path.
#[cfg(feature = "plus")]
type NewPlus<'a> = Box<
    dyn FnOnce(&Logger, &url::Url, Option<JsonPlus>, &Path) -> Result<Plus, super::plus::PlusError>
        + Send
        + 'a,
>;

/// Bencher Plus as the configuration has it, and how to make it.
#[cfg(feature = "plus")]
struct PlusSetup<'a> {
    config: Option<JsonPlus>,
    new: NewPlus<'a>,
}

impl ConfigTx {
    pub async fn into_server<R>(self) -> Result<HttpServer<ApiContext>, ConfigTxError>
    where
        R: Registrar,
    {
        self.into_server_with::<R, _, _>(|| ())
            .await
            .map(|(server, ())| server)
    }

    /// Start the server, running `before_serve` once every check that can refuse a start has
    /// passed, its database migrated, and before the server serves a request or writes from a
    /// task of its own, so a replicator started there misses no write.
    pub async fn into_server_with<R, B, T>(
        self,
        before_serve: B,
    ) -> Result<(HttpServer<ApiContext>, T), ConfigTxError>
    where
        R: Registrar,
        B: FnOnce() -> T,
    {
        let log = into_log(self.config.0.logging.clone())?;
        Box::pin(self.into_inner::<R, _, _>(
            &log,
            before_serve,
            #[cfg(feature = "plus")]
            Box::new(Plus::new),
        ))
        .await
        .inspect_err(|e| error!(&log, "{e}"))
    }

    /// [`Self::into_server_with`], with Bencher Plus configured by `new_plus`.
    async fn into_inner<R, B, T>(
        self,
        log: &Logger,
        before_serve: B,
        #[cfg(feature = "plus")] new_plus: NewPlus<'_>,
    ) -> Result<(HttpServer<ApiContext>, T), ConfigTxError>
    where
        R: Registrar,
        B: FnOnce() -> T,
    {
        let Self { config } = self;

        let Config(JsonConfig {
            console,
            security,
            mut server,
            database,
            smtp,
            logging: _,
            #[cfg(feature = "plus")]
            plus,
        }) = config;

        let request_body_max_bytes = server.request_body_max_bytes;
        #[cfg(feature = "plus")]
        let disaster_recovery = has_disaster_recovery(plus.as_ref());

        debug!(log, "Creating internal configuration");
        let context = into_context(
            log,
            console,
            security,
            request_body_max_bytes,
            smtp,
            database,
            #[cfg(feature = "plus")]
            PlusSetup {
                config: plus,
                new: new_plus,
            },
        )
        .await?;

        // Every check that can refuse a start has passed, and the server has written nothing since
        // its migrations, so a replicator started here misses no request and no write of the
        // server's own.
        let before = before_serve();

        #[cfg(feature = "plus")]
        let daily_stats = daily_stats(log, &context, disaster_recovery).await?;

        debug!(log, "Configuring TLS");
        let tls = server.tls.take().map(|json_tls| match json_tls {
            JsonTls::AsFile {
                cert_file,
                key_file,
            } => ConfigTls::AsFile {
                cert_file,
                key_file,
            },
            JsonTls::AsBytes { certs, key } => ConfigTls::AsBytes { certs, key },
        });
        let config_dropshot = into_config_dropshot(server);

        let mut api_description = ApiDescription::new();
        debug!(log, "Registering server APIs");
        R::register(
            &mut api_description,
            true,
            #[cfg(feature = "plus")]
            context.is_bencher_cloud,
        )
        .map_err(ConfigTxError::Register)?;

        let server = dropshot::HttpServerStarter::new_with_tls(
            &config_dropshot,
            api_description,
            context,
            log,
            tls,
        )
        .map_err(ConfigTxError::CreateServer)?
        .start();

        // The server is already accepting connections. Requests may arrive before job recovery
        // completes: recovery schedules heartbeat timeouts, reprocesses completed jobs, and fires
        // pending callbacks, and a job has at most one delivery, so a runner's resent result and
        // recovery never send its callback twice.
        #[cfg(feature = "plus")]
        spawn_job_recovery(log, server.app_private()).await;

        #[cfg(feature = "plus")]
        daily_stats.spawn();

        Ok((server, before))
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "Context initialization needs to handle DB setup, PRAGMAs, migrations, and pool creation"
)]
async fn into_context(
    log: &Logger,
    console: JsonConsole,
    security: JsonSecurity,
    request_body_max_bytes: usize,
    smtp: Option<JsonSmtp>,
    json_database: JsonDatabase,
    #[cfg(feature = "plus")] plus_setup: PlusSetup<'_>,
) -> Result<ApiContext, ConfigTxError> {
    #[cfg(feature = "plus")]
    let PlusSetup {
        config: plus,
        new: new_plus,
    } = plus_setup;
    let console_url: url::Url = console.url.try_into().map_err(ConfigTxError::Endpoint)?;

    let rbac = init_rbac().map_err(ConfigTxError::Polar)?.into();

    let database_path = json_database.file.to_string_lossy();
    diesel_database_url(log, &database_path);

    sqlite_tmpdir(log, &json_database.file)?;

    info!(log, "Connecting to database: {database_path}");
    let mut database_connection = DbConnection::establish(&database_path)
        .map_err(|e| ConfigTxError::DatabaseConnection(database_path.to_string(), e))?;

    // Set essential SQLite PRAGMAs for concurrent access.
    // WAL mode allows concurrent readers with a single writer.
    // busy_timeout prevents immediate SQLITE_BUSY errors under lock contention.
    // synchronous=NORMAL is safe with WAL mode and reduces fsync overhead.
    let busy_timeout = json_database.busy_timeout.unwrap_or(DEFAULT_BUSY_TIMEOUT);
    let cache_size = json_database.cache_size.unwrap_or(DEFAULT_CACHE_SIZE);
    info!(
        log,
        "Setting database PRAGMAs (busy_timeout: {busy_timeout}ms, cache_size: {cache_size} KiB)"
    );
    database_connection
        .batch_execute("PRAGMA journal_mode = WAL")
        .map_err(ConfigTxError::Pragma)?;
    database_connection
        .batch_execute(&format!("PRAGMA busy_timeout = {busy_timeout}"))
        .map_err(ConfigTxError::Pragma)?;
    database_connection
        .batch_execute("PRAGMA synchronous = NORMAL")
        .map_err(ConfigTxError::Pragma)?;
    // Writer connection only: the read pools hold many connections, so a
    // large per-connection cache would multiply memory, and reads are served
    // by the OS page cache. Negative value = KiB in SQLite.
    database_connection
        .batch_execute(&format!("PRAGMA cache_size = -{cache_size}"))
        .map_err(ConfigTxError::Pragma)?;
    // Surfaces `SQLITE_BUSY_SNAPSHOT` (517) etc. distinctly from plain `SQLITE_BUSY` (5)
    // in error messages, instead of both rendering as "database is locked".
    database_connection
        .batch_execute("PRAGMA extended_result_codes = ON")
        .map_err(ConfigTxError::Pragma)?;

    #[cfg(feature = "plus")]
    let disaster_recovery = has_disaster_recovery(plus.as_ref());
    #[cfg(feature = "plus")]
    if disaster_recovery {
        info!(log, "Configuring Litestream");
        run_litestream(&mut database_connection)?;
    }

    info!(log, "Running database migrations");
    bencher_schema::run_migrations(&mut database_connection)?;

    let public_pool = connection_pool(log, &database_path, busy_timeout)?;
    let auth_pool = connection_pool(log, &database_path, busy_timeout)?;

    let database = Database {
        path: json_database.file,
        busy_timeout,
        public_pool,
        auth_pool,
        connection: Arc::new(tokio::sync::Mutex::new(database_connection)),
    };

    info!(log, "Loading secret key");
    let token_key = TokenKey::new(
        security.issuer.unwrap_or_else(|| console_url.to_string()),
        &security.secret_key,
    );
    #[cfg(feature = "plus")]
    let callback_key =
        CallbackKey::new(&security.secret_key).map_err(ConfigTxError::CallbackKey)?;

    #[cfg(feature = "plus")]
    let rate_limiting = plus.as_ref().and_then(|plus| plus.rate_limiting);

    #[cfg(feature = "plus")]
    let heartbeat_timeout = std::time::Duration::from_secs(
        plus.as_ref()
            .and_then(|p| p.runners.as_ref())
            .map_or(u64::from(DEFAULT_HEARTBEAT_TIMEOUT_SECS), |r| {
                u64::from(u32::from(r.heartbeat_timeout))
            }),
    );
    #[cfg(feature = "plus")]
    let job_timeout_grace_period = std::time::Duration::from_secs(
        plus.as_ref()
            .and_then(|p| p.runners.as_ref())
            .map_or(u64::from(DEFAULT_JOB_TIMEOUT_GRACE_PERIOD_SECS), |r| {
                u64::from(u32::from(r.job_timeout_grace_period))
            }),
    );
    #[cfg(feature = "plus")]
    let runner_update_base_url = plus
        .as_ref()
        .and_then(|p| p.runners.as_ref())
        .and_then(|r| r.update_base_url.clone())
        .map(|url| url.try_into().map_err(ConfigTxError::RunnerUpdateBaseUrl))
        .transpose()?;
    #[cfg(feature = "plus")]
    let openai_apps_challenge = plus
        .as_ref()
        .and_then(|p| p.mcp.as_ref())
        .and_then(|m| m.openai_apps_challenge.clone());
    #[cfg(feature = "plus")]
    let oauth = plus
        .as_ref()
        .and_then(|p| p.oauth.clone())
        .map(bencher_schema::context::OAuthServer::try_from)
        .transpose()
        .map_err(ConfigTxError::OAuthSigningKey)?;

    info!(log, "Configuring Bencher Plus");
    #[cfg(feature = "plus")]
    let Plus {
        github_client,
        github_app,
        google_client,
        stats,
        biller,
        licensor,
        indexer,
        recaptcha_client,
        registry_url,
        oci_storage,
    } = new_plus(log, &console_url, plus, &database.path).map_err(ConfigTxError::Plus)?;

    #[cfg(feature = "plus")]
    let is_bencher_cloud = bencher_json::is_bencher_cloud(&console_url) && biller.is_some();

    let clock = bencher_json::Clock::System;

    // The licenses live in the database, so they are read once every migration has run, and
    // nothing that runs on its own has started yet: the server and replication start after.
    #[cfg(feature = "plus")]
    if disaster_recovery {
        disaster_recovery_gate(
            &mut *database.connection.lock().await,
            &licensor,
            is_bencher_cloud,
            clock.now(),
        )?;
    }

    #[cfg(feature = "plus")]
    let rate_limiting = Arc::new(RateLimiting::new(
        log,
        &mut *database.connection.lock().await,
        &licensor,
        clock.now(),
        is_bencher_cloud,
        rate_limiting,
    ));

    #[cfg(feature = "plus")]
    if let Err(e) = rate_limiting.load(&database.path, log) {
        slog::error!(log, "Failed to restore rate limiting state: {e}");
    }

    // Tripped on graceful shutdown, so the long-lived handlers and the periodic rate limiting task
    // below both wind down instead of running on past the server.
    #[cfg(feature = "plus")]
    let shutdown = bencher_schema::context::CancellationToken::new();

    #[cfg(feature = "plus")]
    let rate_limiting_prune =
        bencher_schema::context::RateLimitingPruneTask::new(RateLimiting::spawn_prune(
            Arc::clone(&rate_limiting),
            database.path.clone(),
            log.clone(),
            shutdown.clone(),
        ));

    #[cfg(feature = "plus")]
    let callbacks = new_callbacks(
        log,
        &database,
        &callback_key,
        &shutdown,
        &clock,
        github_app.clone(),
    );

    debug!(log, "Creating API context");
    Ok(ApiContext {
        console_url,
        request_body_max_bytes,
        token_key,
        rbac,
        messenger: smtp.into(),
        database,
        #[cfg(feature = "plus")]
        rate_limiting,
        #[cfg(feature = "plus")]
        rate_limiting_prune,
        #[cfg(feature = "plus")]
        github_client,
        #[cfg(feature = "plus")]
        github_app,
        #[cfg(feature = "plus")]
        google_client,
        #[cfg(feature = "plus")]
        stats,
        #[cfg(feature = "plus")]
        biller,
        #[cfg(feature = "plus")]
        licensor,
        #[cfg(feature = "plus")]
        indexer,
        #[cfg(feature = "plus")]
        recaptcha_client,
        #[cfg(feature = "plus")]
        is_bencher_cloud,
        #[cfg(feature = "plus")]
        registry_url,
        #[cfg(feature = "plus")]
        oci_storage,
        clock,
        #[cfg(feature = "plus")]
        heartbeat_timeout,
        #[cfg(feature = "plus")]
        job_timeout_grace_period,
        #[cfg(feature = "plus")]
        heartbeat_tasks: bencher_schema::context::HeartbeatTasks::new(),
        #[cfg(feature = "plus")]
        callback_key,
        #[cfg(feature = "plus")]
        callbacks,
        #[cfg(feature = "plus")]
        runner_update: bencher_schema::context::RunnerUpdate::new(runner_update_base_url),
        #[cfg(feature = "plus")]
        openai_apps_challenge,
        #[cfg(feature = "plus")]
        oauth,
        #[cfg(feature = "plus")]
        shutdown,
    })
}

#[cfg(feature = "plus")]
fn new_callbacks(
    log: &Logger,
    database: &Database,
    callback_key: &CallbackKey,
    shutdown: &bencher_schema::context::CancellationToken,
    clock: &bencher_json::Clock,
    github_app: Option<Arc<bencher_github_client::GitHubApp>>,
) -> Callbacks {
    // `main` installed the TLS crypto provider the client builds with.
    let callback_client = CallbackClient::new();
    // A server that cannot send callbacks still serves everything else.
    if let Some(build_error) = callback_client.build_error() {
        error!(
            log,
            "Job callbacks cannot be sent: {}",
            error_chain(build_error)
        );
    }
    Callbacks::new_with_github_app(
        Arc::clone(&database.connection),
        database.auth_pool.clone(),
        callback_key.clone(),
        Arc::new(callback_client),
        shutdown.clone(),
        clock.clone(),
        github_app,
    )
}

// Set the diesel `DATABASE_URL` env var to the database path
// https://diesel.rs/guides/getting-started.html#setup-diesel-for-your-project
fn diesel_database_url(log: &Logger, database_path: &str) {
    if let Ok(database_url) = std::env::var(DATABASE_URL) {
        if database_url == database_path {
            return;
        }
        debug!(
            log,
            "\"{DATABASE_URL}\" ({database_url}) must be the same value as {database_path}"
        );
    } else {
        debug!(log, "Failed to find \"{DATABASE_URL}\"");
    }
    debug!(log, "Setting \"{DATABASE_URL}\" to \"{database_path}\"");
    // SAFETY: This is safe because we are the only process running in production
    // and nothing else is setting the `DATABASE_URL` environment variable
    #[expect(unsafe_code, reason = "DATABASE_URL")]
    unsafe {
        std::env::set_var(DATABASE_URL, database_path);
    }
}

// Set the SQLite `SQLITE_TMPDIR` env var to the temp directory next to the database file.
// This prevents temp files from filling up the root filesystem on containerized deployments.
// Must be called BEFORE establishing any SQLite connections.
// https://www.sqlite.org/tempfiles.html
fn sqlite_tmpdir(log: &Logger, database_path: &Path) -> Result<(), ConfigTxError> {
    // Get the parent directory of the database file and create a tmp subdirectory
    let temp_dir = database_path
        .parent()
        .map_or_else(|| PathBuf::from("tmp"), |p| p.join("tmp"));

    // Create the temp directory if it doesn't exist
    if !temp_dir.exists() {
        info!(log, "Creating SQLite temp directory: {temp_dir:?}");
        fs::create_dir_all(&temp_dir).map_err(|e| ConfigTxError::TempDir(temp_dir.clone(), e))?;
    }

    info!(log, "Setting \"{SQLITE_TMPDIR}\" to {temp_dir:?}");
    // Set the SQLITE_TMPDIR environment variable
    // SQLite checks this env var when determining where to store temporary files
    // SAFETY: This is safe because we are the only process running in production
    // and nothing else is setting the `SQLITE_TMPDIR` environment variable.
    // This must be set before any SQLite connections are established.
    #[expect(unsafe_code, reason = "SQLITE_TMPDIR")]
    unsafe {
        std::env::set_var(SQLITE_TMPDIR, temp_dir.as_os_str());
    }

    Ok(())
}

#[cfg(feature = "plus")]
fn has_disaster_recovery(plus: Option<&JsonPlus>) -> bool {
    plus.is_some_and(|plus| plus.disaster_recovery.is_some())
}

/// Whether a server with disaster recovery configured may start: Bencher Cloud always, without
/// reading its database, and Bencher Self-Hosted with a valid license on any organization at `now`.
#[cfg(feature = "plus")]
fn disaster_recovery_gate(
    conn: &mut DbConnection,
    licensor: &bencher_license::Licensor,
    is_bencher_cloud: bool,
    now: bencher_json::DateTime,
) -> Result<(), ConfigTxError> {
    check_disaster_recovery(is_bencher_cloud, || any_valid_license(conn, licensor, now))
}

/// Whether a server with disaster recovery configured may start, given whether it is Bencher
/// Cloud and, only when it needs to know, whether any organization holds a valid license.
#[cfg(feature = "plus")]
fn check_disaster_recovery(
    is_bencher_cloud: bool,
    licensed: impl FnOnce() -> Result<bool, dropshot::HttpError>,
) -> Result<(), ConfigTxError> {
    if is_bencher_cloud || licensed().map_err(ConfigTxError::DisasterRecoveryCheck)? {
        Ok(())
    } else {
        Err(ConfigTxError::DisasterRecoveryLicense)
    }
}

/// Configure litestream-specific PRAGMAs.
///
/// WAL mode, `busy_timeout`, and `synchronous=NORMAL` are now set unconditionally
/// on all connections. This function only sets the litestream-specific PRAGMA
/// to disable auto-checkpoints (so litestream can manage them).
#[cfg(feature = "plus")]
fn run_litestream(database: &mut DbConnection) -> Result<(), ConfigTxError> {
    // Disable auto-checkpoints
    // https://litestream.io/tips/#disable-autocheckpoints-for-high-write-load-servers
    // https://sqlite.org/wal.html#automatic_checkpoint
    database
        .batch_execute("PRAGMA wal_autocheckpoint = 0")
        .map_err(ConfigTxError::Pragma)?;

    Ok(())
}

/// Sets essential `SQLite` PRAGMAs on every new pool connection.
///
/// `busy_timeout` is per-connection and prevents immediate `SQLITE_BUSY` failures
/// under lock contention. `synchronous = NORMAL` is safe with WAL mode and
/// reduces fsync overhead.
#[derive(Debug)]
struct SqliteConnectionCustomizer {
    busy_timeout: u32,
}

impl diesel::r2d2::CustomizeConnection<DbConnection, diesel::r2d2::Error>
    for SqliteConnectionCustomizer
{
    fn on_acquire(&self, conn: &mut DbConnection) -> Result<(), diesel::r2d2::Error> {
        conn.batch_execute(&format!("PRAGMA busy_timeout = {}", self.busy_timeout))
            .map_err(diesel::r2d2::Error::QueryError)?;
        conn.batch_execute("PRAGMA synchronous = NORMAL")
            .map_err(diesel::r2d2::Error::QueryError)?;
        conn.batch_execute("PRAGMA extended_result_codes = ON")
            .map_err(diesel::r2d2::Error::QueryError)?;
        Ok(())
    }
}

fn connection_pool(
    log: &Logger,
    database_path: &str,
    busy_timeout: u32,
) -> Result<Pool<ConnectionManager<DbConnection>>, ConfigTxError> {
    let cpu_count = std::thread::available_parallelism()
        .map(NonZeroUsize::get)
        .unwrap_or_default();
    // todo(epompeii): Make this configurable
    let max_size = u32::try_from((cpu_count).clamp(2, 8)).unwrap_or(2);
    // todo(epompeii): Make this configurable
    let connection_timeout = std::time::Duration::from_secs(15);
    info!(
        log,
        "Creating database connection pool (max size: {max_size} | connection timeout: {connection_timeout:?})"
    );

    let connection_manager = ConnectionManager::new(database_path);
    let customizer = SqliteConnectionCustomizer { busy_timeout };

    Pool::builder()
        .max_size(max_size)
        .connection_timeout(connection_timeout)
        .connection_customizer(Box::new(customizer))
        .build(connection_manager)
        .map_err(|e| ConfigTxError::DatabaseConnectionPool(database_path.to_owned(), e))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "consumes JsonServer by destructuring"
)]
fn into_config_dropshot(server: JsonServer) -> ConfigDropshot {
    let JsonServer {
        bind_address,
        request_body_max_bytes,
        tls: _,
    } = server;
    ConfigDropshot {
        bind_address,
        default_request_body_max_bytes: request_body_max_bytes,
        default_handler_task_mode: dropshot::HandlerTaskMode::Detached,
        log_headers: Vec::new(),
        compression: dropshot::CompressionConfig::default(),
    }
}

fn into_log(logging: JsonLogging) -> Result<Logger, ConfigTxError> {
    let JsonLogging { name, log } = logging;
    match log {
        ServerLog::StderrTerminal { level } => Ok(bencher_logger::server_logger(
            slog::Level::from(&into_level(&level)),
        )),
        ServerLog::File {
            level,
            path,
            if_exists,
        } => ConfigLogging::File {
            level: into_level(&level),
            path: path.into(),
            if_exists: into_if_exists(&if_exists),
        }
        .to_logger(name)
        .map_err(ConfigTxError::CreateLogger),
    }
}

fn into_level(log_level: &LogLevel) -> ConfigLoggingLevel {
    match log_level {
        LogLevel::Trace => ConfigLoggingLevel::Trace,
        LogLevel::Debug => ConfigLoggingLevel::Debug,
        LogLevel::Info => ConfigLoggingLevel::Info,
        LogLevel::Warn => ConfigLoggingLevel::Warn,
        LogLevel::Error => ConfigLoggingLevel::Error,
        LogLevel::Critical => ConfigLoggingLevel::Critical,
    }
}

fn into_if_exists(if_exists: &IfExists) -> ConfigLoggingIfExists {
    match if_exists {
        IfExists::Fail => ConfigLoggingIfExists::Fail,
        IfExists::Truncate => ConfigLoggingIfExists::Truncate,
        IfExists::Append => ConfigLoggingIfExists::Append,
    }
}

#[cfg(feature = "plus")]
pub async fn spawn_job_recovery(log: &Logger, context: &ApiContext) {
    use bencher_schema::model::runner::{in_flight_jobs, mark_orphaned_claimed_jobs_unknown};

    let in_flight_jobs = {
        let conn = &mut *context.database.connection.lock().await;

        // First, mark any claimed jobs that have been orphaned (claimed longer ago
        // than the heartbeat timeout without transitioning to Running) as Unknown.
        mark_orphaned_claimed_jobs_unknown(log, conn, context.heartbeat_timeout, &context.clock);

        // Then schedule heartbeat timeouts for remaining in-flight jobs.
        match in_flight_jobs(conn) {
            Ok(jobs) => jobs,
            Err(e) => {
                error!(log, "Failed to query in-flight jobs for recovery: {e}");
                Vec::new()
            },
        }
    };

    let count = in_flight_jobs.len();
    if count > 0 {
        info!(
            log,
            "Found {count} in-flight job(s), scheduling heartbeat timeout recovery"
        );
    }

    for job in in_flight_jobs {
        JobTimeout {
            heartbeat: context.heartbeat_timeout,
            grace_period: context.job_timeout_grace_period,
        }
        .spawn(
            log.clone(),
            context.database.connection.clone(),
            job.id,
            &context.heartbeat_tasks,
            context.clock.clone(),
            context.callbacks.clone(),
        );
    }

    // Then reprocess any jobs stuck in Completed (output stored but results not parsed).
    reprocess_completed_jobs(log, context).await;

    // Finally, fire the callbacks a previous process left pending on terminal jobs.
    context.callbacks.fire_pending(log).await;
}

/// The server's daily stats loop, ready to spawn, which first records the server's ID.
#[cfg(feature = "plus")]
async fn daily_stats(
    log: &Logger,
    context: &ApiContext,
    disaster_recovery: bool,
) -> Result<DailyStats, ConfigTxError> {
    let query_server =
        QueryServer::get_or_create(write_conn!(context)).map_err(ConfigTxError::ServerId)?;
    info!(log, "Bencher API Server ID: {}", query_server.uuid);

    query_server
        .daily_stats(
            log.clone(),
            context.database.path.clone(),
            context.database.busy_timeout,
            context.stats,
            context
                .is_bencher_cloud
                .then_some(context.messenger.clone()),
            context.licensor.clone(),
            context.is_bencher_cloud,
            disaster_recovery,
        )
        .map_err(ConfigTxError::SpawnStats)
}

#[cfg(test)]
#[cfg(feature = "plus")]
mod tests {
    use std::{net::SocketAddr, path::Path, time::Duration};

    #[cfg(feature = "test-stub")]
    use bencher_billing::{
        Biller,
        stripe_stub::{StripeStub, StubObjects},
    };
    use bencher_endpoint::Registrar;
    #[cfg(feature = "test-stub")]
    use bencher_github_client::test_server::install_crypto_provider;
    use bencher_json::{
        DateTime, Entitlements, JsonConfig, OrganizationUuid, Secret,
        system::config::{
            JsonConsole, JsonDatabase, JsonDisasterRecovery, JsonLogging, JsonPlus, JsonReplica,
            JsonSecurity, JsonServer, LogLevel, ServerLog,
        },
    };
    use bencher_license::Licensor;
    use bencher_schema::{
        MIGRATIONS,
        context::{ApiContext, DbConnection},
        schema,
    };
    use bencher_token::DEFAULT_SECRET_KEY;
    use diesel::{Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
    use diesel_migrations::MigrationHarness as _;
    use dropshot::{ApiDescription, HttpError, HttpServer};
    use slog::Logger;

    use super::{
        ConfigTx, ConfigTxError, PlusSetup, check_disaster_recovery, disaster_recovery_gate,
        into_context,
    };
    #[cfg(feature = "test-stub")]
    use crate::plus::PlusError;
    use crate::{Config, DEFAULT_MAX_BODY_SIZE, plus::Plus};

    // Debug builds trust this key's public half for Self-Hosted licenses.
    const TEST_LICENSE_KEY: &str =
        include_str!("../../../plus/bencher_license/src/test/private.pem");
    const DAY: i64 = 24 * 60 * 60;

    fn unreadable() -> Result<bool, HttpError> {
        Err(HttpError::for_internal_error(
            "the licenses cannot be read".to_owned(),
        ))
    }

    // Fails if Bencher Cloud reads a license, or refuses, to start with disaster recovery; if a
    // Bencher Self-Hosted server starts with it and no valid license, refuses one with a valid
    // license, or reads a failed license check as a missing license; or if the refusal does not
    // name the license and the configuration key.
    #[test]
    fn disaster_recovery_needs_a_license_on_self_hosted_only() {
        check_disaster_recovery(true, unreadable).unwrap();
        check_disaster_recovery(false, || Ok(true)).unwrap();

        let refused = check_disaster_recovery(false, || Ok(false));
        assert!(
            matches!(refused, Err(ConfigTxError::DisasterRecoveryLicense)),
            "{refused:?}"
        );
        let message = refused.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            message.contains("Bencher Plus Enterprise license"),
            "{message}"
        );
        assert!(message.contains("`plus.disaster_recovery`"), "{message}");

        let failed = check_disaster_recovery(false, unreadable);
        assert!(
            matches!(failed, Err(ConfigTxError::DisasterRecoveryCheck(_))),
            "{failed:?}"
        );
    }

    // Fails if Bencher Cloud, with its own licensor, reads its database to start with disaster
    // recovery, as on a database that holds no license and has no tables to hold one, or if
    // Bencher Self-Hosted starts on that database.
    #[test]
    fn bencher_cloud_starts_with_disaster_recovery_and_no_license() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("unmigrated.db");
        let mut conn = DbConnection::establish(database.to_str().unwrap()).unwrap();
        let key: Secret = TEST_LICENSE_KEY.parse().unwrap();
        let cloud = Licensor::bencher_cloud(&key).unwrap();
        let now = DateTime::now();

        disaster_recovery_gate(&mut conn, &cloud, true, now).unwrap();

        let self_hosted = Licensor::self_hosted().unwrap();
        let refused = disaster_recovery_gate(&mut conn, &self_hosted, false, now);
        assert!(
            matches!(refused, Err(ConfigTxError::DisasterRecoveryCheck(_))),
            "{refused:?}"
        );
    }

    /// Bencher Plus with its database in `dir`, replicating to a file in `dir` when
    /// `disaster_recovery`, and no `cloud` section.
    fn plus(dir: &Path, disaster_recovery: bool) -> JsonPlus {
        JsonPlus {
            rate_limiting: None,
            github: None,
            google: None,
            disaster_recovery: disaster_recovery.then(|| JsonDisasterRecovery {
                replica: JsonReplica::File {
                    path: dir.join("replica"),
                    sync_interval: None,
                },
                snapshot: None,
                validation: None,
                checkpoint: None,
            }),
            stats: None,
            cloud: None,
            registry: None,
            runners: None,
            mcp: None,
            oauth: None,
        }
    }

    fn database(dir: &Path) -> JsonDatabase {
        JsonDatabase {
            file: dir.join("bencher.db"),
            busy_timeout: None,
            cache_size: None,
        }
    }

    /// The context of a Bencher Self-Hosted server with its database in `dir`, replicating to
    /// a file in `dir` when `disaster_recovery`.
    async fn self_hosted(dir: &Path, disaster_recovery: bool) -> Result<ApiContext, ConfigTxError> {
        let log = Logger::root(slog::Discard, slog::o!());
        into_context(
            &log,
            JsonConsole {
                url: "http://localhost:3000".parse().unwrap(),
            },
            JsonSecurity {
                issuer: None,
                secret_key: DEFAULT_SECRET_KEY.clone(),
            },
            DEFAULT_MAX_BODY_SIZE,
            None,
            database(dir),
            PlusSetup {
                config: Some(plus(dir, disaster_recovery)),
                new: Box::new(Plus::new),
            },
        )
        .await
    }

    /// The configuration of a server at `bind_address` with its database in `dir`, replicating
    /// to a file in `dir` when `disaster_recovery`. Its console URL is Bencher Cloud's in a debug
    /// build, so a biller makes it Bencher Cloud.
    fn config(dir: &Path, disaster_recovery: bool, bind_address: SocketAddr) -> ConfigTx {
        ConfigTx {
            config: Config(JsonConfig {
                console: JsonConsole {
                    url: "http://localhost:3000".parse().unwrap(),
                },
                security: JsonSecurity {
                    issuer: None,
                    secret_key: DEFAULT_SECRET_KEY.clone(),
                },
                server: JsonServer {
                    bind_address,
                    request_body_max_bytes: DEFAULT_MAX_BODY_SIZE,
                    tls: None,
                },
                database: database(dir),
                smtp: None,
                logging: JsonLogging {
                    name: "bencher_config_test".to_owned(),
                    log: ServerLog::StderrTerminal {
                        level: LogLevel::Error,
                    },
                },
                plus: Some(plus(dir, disaster_recovery)),
            }),
        }
    }

    /// A server with no endpoints, which answers every request with 404.
    struct NoApi;

    impl Registrar for NoApi {
        fn register(
            _api_description: &mut ApiDescription<ApiContext>,
            _http_options: bool,
            _is_bencher_cloud: bool,
        ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
            Ok(())
        }
    }

    /// An address on loopback that nothing holds now.
    fn free_address() -> SocketAddr {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
    }

    /// Whether anything accepts a connection at `address`.
    fn accepts(address: SocketAddr) -> bool {
        std::net::TcpStream::connect_timeout(&address, Duration::from_millis(500)).is_ok()
    }

    /// The rows of the server table, which the server writes as it starts.
    fn server_rows(dir: &Path) -> i64 {
        let mut conn = DbConnection::establish(dir.join("bencher.db").to_str().unwrap()).unwrap();
        schema::server::table.count().get_result(&mut conn).unwrap()
    }

    fn license_an_organization(dir: &Path) {
        let mut conn = DbConnection::establish(dir.join("bencher.db").to_str().unwrap()).unwrap();
        bencher_schema::run_migrations(&mut conn).unwrap();
        let organization = OrganizationUuid::new();
        let now = DateTime::now();
        let at = |seconds: i64| DateTime::try_from(now.timestamp() + seconds).unwrap();
        let key: Secret = TEST_LICENSE_KEY.parse().unwrap();
        let license = Licensor::bencher_cloud(&key)
            .unwrap()
            .new_license(organization, Entitlements::MIN, at(-DAY), at(365 * DAY))
            .unwrap();
        diesel::insert_into(schema::organization::table)
            .values((
                schema::organization::uuid.eq(organization),
                schema::organization::name.eq("Licensed"),
                schema::organization::slug.eq("licensed"),
                schema::organization::license.eq(&license),
                schema::organization::created.eq(&now),
                schema::organization::modified.eq(&now),
            ))
            .execute(&mut conn)
            .unwrap();
    }

    async fn close(server: HttpServer<ApiContext>) {
        server.app_private().shutdown.cancel();
        server.close().await.unwrap();
    }

    // Fails if replication starts before the server has passed its checks and migrated its
    // database, or once the server serves a request or has written from a task of its own; if a
    // refused start replicates; or if the server does not serve once replication has started.
    #[tokio::test]
    async fn replication_starts_after_the_checks_and_before_the_server_serves() {
        let dir = tempfile::tempdir().unwrap();
        let address = free_address();
        let mut replicated = false;
        let refused = config(dir.path(), true, address)
            .into_server_with::<NoApi, _, _>(|| replicated = true)
            .await;
        assert!(
            matches!(refused, Err(ConfigTxError::DisasterRecoveryLicense)),
            "{:?}",
            refused.err()
        );
        assert!(!replicated, "a refused start replicated");

        license_an_organization(dir.path());
        let (server, (migrated, rows, accepted)) = config(dir.path(), true, address)
            .into_server_with::<NoApi, _, _>(|| {
                let mut conn =
                    DbConnection::establish(dir.path().join("bencher.db").to_str().unwrap())
                        .unwrap();
                let migrated = !conn.has_pending_migration(MIGRATIONS).unwrap();
                (migrated, server_rows(dir.path()), accepts(address))
            })
            .await
            .unwrap();
        assert!(migrated, "replication started before the migrations");
        assert_eq!(rows, 0, "the server wrote before replication started");
        assert!(!accepted, "the server served before replication started");
        assert_eq!(server_rows(dir.path()), 1);
        assert!(accepts(address), "the server does not serve");
        close(server).await;
    }

    // Fails if a Bencher Cloud server, as the real start makes one from its console URL and its
    // biller, does not start and replicate with disaster recovery and no license.
    #[cfg(feature = "test-stub")]
    #[tokio::test]
    async fn bencher_cloud_starts_and_replicates_without_a_license() {
        install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let stub = StripeStub::new(StubObjects::default()).await;
        let biller = Biller::for_stub(&stub).unwrap();
        let key: Secret = TEST_LICENSE_KEY.parse().unwrap();
        let licensor = Licensor::bencher_cloud(&key).unwrap();
        let new_plus = move |log: &Logger,
                             console_url: &url::Url,
                             plus: Option<JsonPlus>,
                             database_path: &Path|
              -> Result<Plus, PlusError> {
            let mut built = Plus::new(log, console_url, plus, database_path)?;
            built.biller = Some(biller);
            built.licensor = licensor;
            Ok(built)
        };
        let log = Logger::root(slog::Discard, slog::o!());
        let mut replicated = false;
        let (server, ()) = Box::pin(
            config(dir.path(), true, free_address()).into_inner::<NoApi, _, _>(
                &log,
                || replicated = true,
                Box::new(new_plus),
            ),
        )
        .await
        .unwrap();
        assert!(server.app_private().is_bencher_cloud);
        assert!(replicated, "Bencher Cloud did not replicate");
        close(server).await;
    }

    // Fails if a server that cannot bind its address does not say so, or a start refused for
    // another reason says it could not bind.
    #[tokio::test]
    async fn only_a_bind_failure_is_a_bind_error() {
        let dir = tempfile::tempdir().unwrap();
        let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let bound = config(dir.path(), false, held.local_addr().unwrap())
            .into_server::<NoApi>()
            .await;
        let Err(bound) = bound else {
            panic!("A server started at an address another holds");
        };
        assert!(bound.is_bind_error(), "{bound}");

        let refused = config(dir.path(), true, free_address())
            .into_server::<NoApi>()
            .await;
        let Err(refused) = refused else {
            panic!("A server with disaster recovery and no license started");
        };
        assert!(!refused.is_bind_error(), "{refused}");
    }

    fn stop(context: &ApiContext) {
        context.shutdown.cancel();
    }

    // Fails if a Bencher Self-Hosted server with disaster recovery and no valid license starts,
    // refuses before its database has every migration, or refuses to start with a valid license
    // or without disaster recovery.
    #[tokio::test]
    async fn self_hosted_disaster_recovery_refuses_to_start_without_a_license() {
        let dir = tempfile::tempdir().unwrap();

        let refused = self_hosted(dir.path(), true).await;
        let Err(refused) = refused else {
            panic!("A server with disaster recovery and no license started");
        };
        assert!(
            matches!(refused, ConfigTxError::DisasterRecoveryLicense),
            "{refused}"
        );
        let database = dir.path().join("bencher.db");
        let mut conn = DbConnection::establish(database.to_str().unwrap()).unwrap();
        assert!(!conn.has_pending_migration(MIGRATIONS).unwrap());

        let unreplicated = self_hosted(dir.path(), false).await.unwrap();
        stop(&unreplicated);

        let organization = OrganizationUuid::new();
        let now = DateTime::now();
        let at = |seconds: i64| DateTime::try_from(now.timestamp() + seconds).unwrap();
        let key: Secret = TEST_LICENSE_KEY.parse().unwrap();
        let license = Licensor::bencher_cloud(&key)
            .unwrap()
            .new_license(organization, Entitlements::MIN, at(-DAY), at(365 * DAY))
            .unwrap();
        diesel::insert_into(schema::organization::table)
            .values((
                schema::organization::uuid.eq(organization),
                schema::organization::name.eq("Licensed"),
                schema::organization::slug.eq("licensed"),
                schema::organization::license.eq(&license),
                schema::organization::created.eq(&now),
                schema::organization::modified.eq(&now),
            ))
            .execute(&mut conn)
            .unwrap();
        let licensed = self_hosted(dir.path(), true).await.unwrap();
        stop(&licensed);
    }
}
