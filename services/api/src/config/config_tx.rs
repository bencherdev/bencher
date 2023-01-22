use std::convert::TryFrom;

use bencher_json::{
    system::config::{
        IfExists, JsonDatabase, JsonLogging, JsonServer, JsonSmtp, JsonTls, LogLevel, ServerLog,
    },
    JsonConfig,
};
use bencher_rbac::init_rbac;
use diesel::{Connection, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dropshot::{
    ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingIfExists, ConfigLoggingLevel,
    ConfigTls, HttpServer,
};
use slog::Logger;
use tokio::sync::{mpsc::Sender, Mutex};
use tracing::trace;
use url::Url;

use crate::{
    context::{ApiContext, Context, Email, Messenger, SecretKey},
    endpoints::Api,
    util::registrar::Registrar,
    ApiError,
};

use super::Config;

const DATABASE_URL: &str = "DATABASE_URL";
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

pub struct ConfigTx {
    pub config: Config,
    pub restart_tx: Sender<()>,
}

impl TryFrom<ConfigTx> for HttpServer<Context> {
    type Error = ApiError;

    fn try_from(config_tx: ConfigTx) -> Result<Self, Self::Error> {
        let ConfigTx { config, restart_tx } = config_tx;

        let Config(JsonConfig {
            endpoint,
            secret_key,
            server,
            database,
            smtp,
            logging,
        }) = config;

        let private = into_private(endpoint, secret_key, smtp, database, restart_tx)?;
        let config_dropshot = into_config_dropshot(server);
        let log = into_log(logging)?;

        let mut api = ApiDescription::new();
        trace!("Registering server APIs");
        Api::register(&mut api)?;

        Ok(
            dropshot::HttpServerStarter::new(&config_dropshot, api, private, &log)
                .map_err(ApiError::CreateServer)?
                .start(),
        )
    }
}

fn into_private(
    endpoint: Url,
    secret_key: Option<String>,
    smtp: Option<JsonSmtp>,
    json_database: JsonDatabase,
    restart_tx: Sender<()>,
) -> Result<Mutex<ApiContext>, ApiError> {
    let database_path = json_database.file.to_string_lossy();
    diesel_database_url(&database_path);
    let mut database = SqliteConnection::establish(&database_path)?;
    run_migrations(&mut database)?;
    Ok(Mutex::new(ApiContext {
        endpoint,
        secret_key: into_secret_key(secret_key),
        rbac: init_rbac().map_err(ApiError::Polar)?.into(),
        messenger: into_messenger(smtp),
        database,
        database_path: json_database.file,
        restart_tx,
    }))
}

// Set the diesel `DATABASE_URL` env var to the database path
fn diesel_database_url(database_path: &str) {
    if let Ok(database_url) = std::env::var(DATABASE_URL) {
        if database_url == database_path {
            return;
        }
        trace!("\"{DATABASE_URL}\" ({database_url}) must be the same value as {database_path}");
    } else {
        trace!("Failed to find \"{DATABASE_URL}\"");
    }
    trace!("Setting \"{DATABASE_URL}\" to {database_path}");
    std::env::set_var(DATABASE_URL, database_path);
}

fn run_migrations(database: &mut SqliteConnection) -> Result<(), ApiError> {
    database
        .run_pending_migrations(MIGRATIONS)
        .map(|_| ())
        .map_err(ApiError::Migrations)
}

fn into_secret_key(secret_key: Option<String>) -> SecretKey {
    secret_key
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
        .into()
}

fn into_messenger(smtp: Option<JsonSmtp>) -> Messenger {
    smtp.map_or(
        Messenger::StdOut,
        |JsonSmtp {
             hostname,
             username,
             secret,
             from_name,
             from_email,
         }| {
            Messenger::Email(Email {
                hostname,
                username,
                secret,
                from_name: Some(from_name),
                from_email,
            })
        },
    )
}

fn into_config_dropshot(server: JsonServer) -> ConfigDropshot {
    let JsonServer {
        bind_address,
        request_body_max_bytes,
        tls,
    } = server;
    ConfigDropshot {
        bind_address,
        request_body_max_bytes,
        tls: tls.map(
            |JsonTls {
                 cert_file,
                 key_file,
             }| ConfigTls {
                cert_file,
                key_file,
            },
        ),
    }
}

fn into_log(logging: JsonLogging) -> Result<Logger, ApiError> {
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
            path,
            if_exists: into_if_exists(&if_exists),
        },
    }
    .to_logger(name)
    .map_err(ApiError::CreateLogger)
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
