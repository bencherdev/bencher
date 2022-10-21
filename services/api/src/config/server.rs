use std::convert::TryFrom;

use bencher_json::{
    config::{IfExists, JsonLogging, JsonServer, JsonSmtp, JsonTls, LogLevel, ServerLog},
    JsonConfig,
};
use bencher_rbac::init_rbac;
use diesel::{Connection, SqliteConnection};
use dropshot::{
    ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingIfExists, ConfigLoggingLevel,
    ConfigTls, HttpServer,
};
use tokio::sync::Mutex;
use tracing::trace;

use crate::{
    endpoints::Api,
    util::{
        context::{Email, Messenger},
        registrar::Registrar,
        ApiContext, Context,
    },
    ApiError,
};

use super::Config;

const DATABASE_URL: &str = "DATABASE_URL";

impl TryFrom<Config> for HttpServer<Context> {
    type Error = ApiError;

    fn try_from(config: Config) -> Result<Self, Self::Error> {
        let Config(JsonConfig {
            endpoint,
            secret_key,
            server,
            database,
            smtp,
            logging,
        }) = config;

        let JsonServer {
            bind_address,
            request_body_max_bytes,
            tls,
        } = server;
        let config_dropshot = ConfigDropshot {
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
        };

        let database_path = database.file.to_string_lossy();
        diesel_database_url(&database_path);
        let private = Mutex::new(ApiContext {
            endpoint,
            secret_key: secret_key
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
                .into(),
            rbac: init_rbac().map_err(ApiError::Polar)?.into(),
            messenger: smtp
                .map(
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
                .unwrap_or(Messenger::StdOut),
            database: SqliteConnection::establish(&database_path)?,
        });

        let JsonLogging { name, log } = logging;
        fn map_level(log_level: LogLevel) -> ConfigLoggingLevel {
            match log_level {
                LogLevel::Trace => ConfigLoggingLevel::Trace,
                LogLevel::Debug => ConfigLoggingLevel::Debug,
                LogLevel::Info => ConfigLoggingLevel::Info,
                LogLevel::Warn => ConfigLoggingLevel::Warn,
                LogLevel::Error => ConfigLoggingLevel::Error,
                LogLevel::Critical => ConfigLoggingLevel::Critical,
            }
        }
        fn map_if_exists(if_exists: IfExists) -> ConfigLoggingIfExists {
            match if_exists {
                IfExists::Fail => ConfigLoggingIfExists::Fail,
                IfExists::Truncate => ConfigLoggingIfExists::Truncate,
                IfExists::Append => ConfigLoggingIfExists::Append,
            }
        }
        let log = match log {
            ServerLog::StderrTerminal { level } => ConfigLogging::StderrTerminal {
                level: map_level(level),
            },
            ServerLog::File {
                level,
                path,
                if_exists,
            } => ConfigLogging::File {
                level: map_level(level),
                path,
                if_exists: map_if_exists(if_exists),
            },
        }
        .to_logger(name)
        .map_err(ApiError::CreateLogger)?;

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
    std::env::set_var(DATABASE_URL, database_path)
}
