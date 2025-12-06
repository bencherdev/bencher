use std::sync::Arc;

use bencher_endpoint::Registrar;
#[cfg(feature = "plus")]
use bencher_json::system::config::{JsonLitestream, JsonPlus};
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
use bencher_schema::{context::RateLimiting, model::server::QueryServer};
use bencher_token::TokenKey;
use diesel::Connection as _;
#[cfg(feature = "plus")]
use diesel::connection::SimpleConnection as _;
use dropshot::{
    ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingIfExists, ConfigLoggingLevel,
    ConfigTls, HttpServer,
};
use slog::{Logger, debug, error, info};
use tokio::sync::mpsc::Sender;

use super::Config;
#[cfg(feature = "plus")]
use super::{DEFAULT_BUSY_TIMEOUT, plus::Plus};

const DATABASE_URL: &str = "DATABASE_URL";

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
    #[error("Failed to parse role based access control (RBAC) rules: {0}")]
    Polar(Box<oso::OsoError>),
    #[error("Invalid endpoint URL: {0}")]
    Endpoint(bencher_json::ValidError),
    #[error("Failed to connect to database ({0}): {1}")]
    DatabaseConnection(String, diesel::ConnectionError),
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
    #[error("Failed to configure rate limits: {0}")]
    RateLimiting(Box<bencher_schema::context::RateLimitingError>),
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

        debug!(log, "Creating internal configuration");
        let context = into_context(
            log,
            console,
            security,
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

        // Bencher Cloud does not need to send stats, it uses OpenTelemetry.
        #[cfg(feature = "plus")]
        if !context.is_bencher_cloud {
            register_startup(log).await;
            spawn_stats(log.clone(), &context).await?;
        }

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

async fn into_context(
    log: &Logger,
    console: JsonConsole,
    security: JsonSecurity,
    smtp: Option<JsonSmtp>,
    json_database: JsonDatabase,
    restart_tx: Sender<()>,
    #[cfg(feature = "plus")] plus: Option<JsonPlus>,
) -> Result<ApiContext, ConfigTxError> {
    let console_url: url::Url = console.url.try_into().map_err(ConfigTxError::Endpoint)?;

    let rbac = init_rbac().map_err(ConfigTxError::Polar)?.into();

    let database_path = json_database.file.to_string_lossy();
    diesel_database_url(log, &database_path);

    info!(&log, "Connecting to database: {database_path}");
    let mut database_connection = DbConnection::establish(&database_path)
        .map_err(|e| ConfigTxError::DatabaseConnection(database_path.to_string(), e))?;

    #[cfg(feature = "plus")]
    if let Some(litestream) = plus.as_ref().and_then(|plus| plus.litestream.as_ref()) {
        info!(&log, "Configuring Litestream");
        run_litestream(&mut database_connection, litestream)?;
    }

    info!(&log, "Running database migrations");
    bencher_schema::run_migrations(&mut database_connection)?;

    let data_store = if let Some(data_store) = json_database.data_store {
        Some(data_store.try_into().map_err(ConfigTxError::DataStore)?)
    } else {
        None
    };

    let database = Database {
        path: json_database.file,
        connection: Arc::new(tokio::sync::Mutex::new(database_connection)),
        data_store,
    };

    info!(&log, "Loading secret key");
    let token_key = TokenKey::new(
        security.issuer.unwrap_or_else(|| console_url.to_string()),
        &security.secret_key,
    );

    #[cfg(feature = "plus")]
    let rate_limiting = plus.as_ref().and_then(|plus| plus.rate_limiting);

    info!(&log, "Configuring Bencher Plus");
    #[cfg(feature = "plus")]
    let Plus {
        github_client,
        google_client,
        stats,
        biller,
        licensor,
        indexer,
        recaptcha_client,
    } = Plus::new(&console_url, plus).map_err(ConfigTxError::Plus)?;

    #[cfg(feature = "plus")]
    let is_bencher_cloud = bencher_json::is_bencher_cloud(&console_url) && biller.is_some();

    #[cfg(feature = "plus")]
    let rate_limiting = RateLimiting::new(
        log,
        &database.connection,
        &licensor,
        is_bencher_cloud,
        rate_limiting,
    )
    .await
    .map_err(Box::new)
    .map_err(ConfigTxError::RateLimiting)?;

    debug!(&log, "Creating API context");
    Ok(ApiContext {
        console_url,
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
    })
}

// Set the diesel `DATABASE_URL` env var to the database path
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
    debug!(log, "Setting \"{DATABASE_URL}\" to {database_path}");
    // SAFETY: This is safe because we are the only process running in production
    // and nothing else is setting the `DATABASE_URL` environment variable
    #[expect(unsafe_code, reason = "DATABASE_URL")]
    unsafe {
        std::env::set_var(DATABASE_URL, database_path);
    }
}

#[cfg(feature = "plus")]
fn run_litestream(
    database: &mut DbConnection,
    litestream: &JsonLitestream,
) -> Result<(), ConfigTxError> {
    // Enable WAL mode
    // https://litestream.io/tips/#wal-journal-mode
    // https://sqlite.org/wal.html
    database
        .batch_execute("PRAGMA journal_mode = WAL")
        .map_err(ConfigTxError::Pragma)?;
    // Disable auto-checkpoints
    // https://litestream.io/tips/#disable-autocheckpoints-for-high-write-load-servers
    // https://sqlite.org/wal.html#automatic_checkpoint
    database
        .batch_execute("PRAGMA wal_autocheckpoint = 0")
        .map_err(ConfigTxError::Pragma)?;
    // Enable busy timeout
    // https://litestream.io/tips/#busy-timeout
    // https://www.sqlite.org/pragma.html#pragma_busy_timeout
    let busy_timeout = format!(
        "PRAGMA busy_timeout = {}",
        litestream.busy_timeout.unwrap_or(DEFAULT_BUSY_TIMEOUT)
    );
    database
        .batch_execute(&busy_timeout)
        .map_err(ConfigTxError::Pragma)?;
    // Relax synchronous mode because we are using WAL mode
    // https://litestream.io/tips/#synchronous-pragma
    // https://www.sqlite.org/pragma.html#pragma_synchronous
    database
        .batch_execute("PRAGMA synchronous = NORMAL")
        .map_err(ConfigTxError::Pragma)?;

    Ok(())
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
async fn register_startup(log: &Logger) {
    let client = reqwest::Client::new();
    if let Err(e) = client
        .get(bencher_json::BENCHER_STATS_API_URL.clone())
        .query(&bencher_json::SelfHostedStartup)
        .send()
        .await
    {
        slog::warn!(log, "Failed to register startup: {e}");
    }
}

#[cfg(feature = "plus")]
async fn spawn_stats(log: Logger, context: &ApiContext) -> Result<(), ConfigTxError> {
    let query_server = QueryServer::get_or_create(bencher_schema::conn_lock!(context))
        .map_err(ConfigTxError::ServerId)?;
    info!(log, "Bencher API Server ID: {}", query_server.uuid);

    query_server.spawn_stats(
        log.clone(),
        context.database.path.clone(),
        context.database.connection.clone(),
        context.stats,
        context.licensor.clone(),
    );

    Ok(())
}
