use std::{convert::TryFrom, path::Path};

use bencher_json::{
    config::{JsonServer, JsonSmtp, JsonTls},
    JsonConfig,
};
use bencher_rbac::init_rbac;
use diesel::{Connection, SqliteConnection};
use dropshot::{ConfigDropshot, ConfigTls, HttpServer};
use tokio::sync::Mutex;
use tracing::trace;

use crate::{
    util::{
        context::{Email, Messenger},
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
