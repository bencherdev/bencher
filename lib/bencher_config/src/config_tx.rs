use std::{
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
};

use bencher_endpoint::Registrar;
#[cfg(feature = "plus")]
use bencher_json::system::config::JsonPlus;
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
    context::RateLimiting,
    model::{runner::job::spawn_heartbeat_timeout, server::QueryServer},
    write_conn,
};
use bencher_token::TokenKey;
use diesel::{
    Connection as _,
    connection::SimpleConnection as _,
    r2d2::{ConnectionManager, Pool},
};
#[cfg(feature = "plus")]
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{
    ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingIfExists, ConfigLoggingLevel,
    ConfigTls, HttpServer,
};
use slog::{Logger, debug, error, info};
use tokio::sync::mpsc::Sender;

#[cfg(feature = "plus")]
use super::plus::Plus;
use super::{Config, DEFAULT_BUSY_TIMEOUT};

const DATABASE_URL: &str = "DATABASE_URL";
const SQLITE_TMPDIR: &str = "SQLITE_TMPDIR";

pub struct ConfigTx {
    pub config: Config,
    pub restart_tx: Sender<()>,
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
    #[error("Failed to parse data store: {0}")]
    DataStore(bencher_schema::context::DataStoreError),
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
    #[error("Failed to spawn stats: {0}")]
    SpawnStats(dropshot::HttpError),
}

impl ConfigTx {
    pub async fn into_server<R>(self) -> Result<HttpServer<ApiContext>, ConfigTxError>
    where
        R: Registrar,
    {
        let log = into_log(self.config.0.logging.clone())?;
        Box::pin(self.into_inner::<R>(&log))
            .await
            .inspect_err(|e| error!(&log, "{e}"))
    }

    async fn into_inner<R>(self, log: &Logger) -> Result<HttpServer<ApiContext>, ConfigTxError>
    where
        R: Registrar,
    {
        let ConfigTx { config, restart_tx } = self;

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

        debug!(log, "Creating internal configuration");
        let context = into_context(
            log,
            console,
            security,
            request_body_max_bytes,
            smtp,
            database,
            restart_tx,
            #[cfg(feature = "plus")]
            plus,
        )
        .await?;
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

        #[cfg(feature = "plus")]
        spawn_stats(log.clone(), &context).await?;

        #[cfg(feature = "plus")]
        spawn_job_recovery(log.clone(), &context).await;

        let mut api_description = ApiDescription::new();
        debug!(log, "Registering server APIs");
        R::register(
            &mut api_description,
            true,
            #[cfg(feature = "plus")]
            context.is_bencher_cloud,
        )
        .map_err(ConfigTxError::Register)?;

        Ok(dropshot::HttpServerStarter::new_with_tls(
            &config_dropshot,
            api_description,
            context,
            log,
            tls,
        )
        .map_err(ConfigTxError::CreateServer)?
        .start())
    }
}

#[expect(
    clippy::too_many_arguments,
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
    restart_tx: Sender<()>,
    #[cfg(feature = "plus")] plus: Option<JsonPlus>,
) -> Result<ApiContext, ConfigTxError> {
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
    info!(
        log,
        "Setting database PRAGMAs (busy_timeout: {busy_timeout}ms)"
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

    #[cfg(feature = "plus")]
    if plus
        .as_ref()
        .and_then(|plus| plus.litestream.as_ref())
        .is_some()
    {
        info!(log, "Configuring Litestream");
        run_litestream(&mut database_connection)?;
    }

    info!(log, "Running database migrations");
    bencher_schema::run_migrations(&mut database_connection)?;

    let public_pool = connection_pool(log, &database_path, busy_timeout)?;
    let auth_pool = connection_pool(log, &database_path, busy_timeout)?;

    let data_store = if let Some(data_store) = json_database.data_store {
        Some(data_store.try_into().map_err(ConfigTxError::DataStore)?)
    } else {
        None
    };

    let database = Database {
        path: json_database.file,
        public_pool,
        auth_pool,
        connection: Arc::new(tokio::sync::Mutex::new(database_connection)),
        data_store,
    };

    info!(log, "Loading secret key");
    let token_key = TokenKey::new(
        security.issuer.unwrap_or_else(|| console_url.to_string()),
        &security.secret_key,
    );

    #[cfg(feature = "plus")]
    let rate_limiting = plus.as_ref().and_then(|plus| plus.rate_limiting);

    info!(log, "Configuring Bencher Plus");
    #[cfg(feature = "plus")]
    let Plus {
        github_client,
        google_client,
        stats,
        biller,
        licensor,
        indexer,
        recaptcha_client,
        oci_storage,
    } = Plus::new(log, &console_url, plus, &database.path).map_err(ConfigTxError::Plus)?;

    #[cfg(feature = "plus")]
    let is_bencher_cloud = bencher_json::is_bencher_cloud(&console_url) && biller.is_some();

    #[cfg(feature = "plus")]
    let rate_limiting = RateLimiting::new(
        log,
        &mut *database.connection.lock().await,
        &licensor,
        is_bencher_cloud,
        rate_limiting,
    );

    debug!(log, "Creating API context");
    Ok(ApiContext {
        console_url,
        request_body_max_bytes,
        token_key,
        rbac,
        messenger: smtp.into(),
        database,
        restart_tx,
        #[cfg(feature = "plus")]
        rate_limiting,
        #[cfg(feature = "plus")]
        github_client,
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
        oci_storage,
        #[cfg(feature = "plus")]
        heartbeat_timeout: std::time::Duration::from_secs(90),
        #[cfg(feature = "plus")]
        heartbeat_tasks: bencher_schema::context::HeartbeatTasks::new(),
    })
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

#[expect(clippy::needless_pass_by_value)]
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
    }
}

fn into_log(logging: JsonLogging) -> Result<Logger, ConfigTxError> {
    let JsonLogging { name, log } = logging;
    match log {
        ServerLog::StderrTerminal { level } => ConfigLogging::StderrTerminal {
            level: into_level(&level),
        },
        ServerLog::File {
            level,
            path,
            if_exists,
        } => ConfigLogging::File {
            level: into_level(&level),
            path: path.into(),
            if_exists: into_if_exists(&if_exists),
        },
    }
    .to_logger(name)
    .map_err(ConfigTxError::CreateLogger)
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
async fn spawn_job_recovery(log: Logger, context: &ApiContext) {
    use bencher_json::JobStatus;
    use bencher_schema::{model::runner::QueryJob, schema};
    use diesel::BoolExpressionMethods as _;

    let conn = &mut *context.database.connection.lock().await;
    let in_flight_jobs: Vec<QueryJob> = match schema::job::table
        .filter(
            schema::job::status
                .eq(JobStatus::Claimed)
                .or(schema::job::status.eq(JobStatus::Running)),
        )
        .load(conn)
    {
        Ok(jobs) => jobs,
        Err(e) => {
            error!(log, "Failed to query in-flight jobs for recovery: {e}");
            return;
        },
    };

    let count = in_flight_jobs.len();
    if count > 0 {
        info!(
            log,
            "Found {count} in-flight job(s), scheduling heartbeat timeout recovery"
        );
    }

    for job in in_flight_jobs {
        spawn_heartbeat_timeout(
            log.clone(),
            context.heartbeat_timeout,
            context.database.connection.clone(),
            job.id,
            &context.heartbeat_tasks,
        );
    }
}

#[cfg(feature = "plus")]
async fn spawn_stats(log: Logger, context: &ApiContext) -> Result<(), ConfigTxError> {
    let query_server =
        QueryServer::get_or_create(write_conn!(context)).map_err(ConfigTxError::ServerId)?;
    info!(log, "Bencher API Server ID: {}", query_server.uuid);

    query_server
        .spawn_stats(
            log.clone(),
            context.database.path.clone(),
            context.stats,
            context
                .is_bencher_cloud
                .then_some(context.messenger.clone()),
            context.licensor.clone(),
            context.is_bencher_cloud,
        )
        .map_err(ConfigTxError::SpawnStats)
}
