use diesel::{Connection, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dropshot::{ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, HttpServer};
use tokio::sync::Mutex;
use tracing::{info, trace};

use super::{registrar::Registrar, ApiContext, Context};
use crate::{endpoints::Api, ApiError};

const BENCHER_SECRET_KEY: &str = "BENCHER_SECRET_KEY";
const BENCHER_DB: &str = "BENCHER_DB";
const DATABASE_URL: &str = "DATABASE_URL";

const PORT_KEY: &str = "BENCHER_PORT";
const DEFAULT_IP: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "8080";
const DEFAULT_DB: &str = "bencher.db";

// TODO increase and add as a customizable feature
// 1 megabyte or 1_048_576 bytes
const MAX_BODY_SIZE: usize = 1 << 20;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

pub async fn get_server(api_name: &str) -> Result<HttpServer<Context>, ApiError> {
    trace!("Setting secret key");
    let secret_key = get_secret();
    trace!("Getting database connection");
    let mut conn = get_db()?;
    trace!("Running database migrations");
    run_migrations(&mut conn)?;
    let private = Mutex::new(ApiContext {
        key: secret_key,
        db: conn,
    });

    trace!("Getting server configuration");
    let config = get_config();
    let mut api = ApiDescription::new();
    trace!("Registering server APIs");
    Api::register(&mut api)?;
    trace!("Creating server logger");
    let log = get_logger(api_name)?;

    trace!("Creating server");
    Ok(
        dropshot::HttpServerStarter::new(&config, api, private, &log)
            .map_err(ApiError::CreateServer)?
            .start(),
    )
}

fn get_secret() -> String {
    std::env::var(BENCHER_SECRET_KEY).unwrap_or_else(|e| {
        info!("Failed to find \"{BENCHER_SECRET_KEY}\": {e}");
        let secret_key = uuid::Uuid::new_v4().to_string();
        info!("Generated temporary secret key: {secret_key}");
        secret_key
    })
}

fn get_db() -> Result<SqliteConnection, ApiError> {
    let db = std::env::var(BENCHER_DB).unwrap_or_else(|e| {
        info!("Failed to find \"{BENCHER_DB}\": {e}");
        info!("Defaulting \"{BENCHER_DB}\" to: {DEFAULT_DB}");
        DEFAULT_DB.into()
    });
    diesel_database_url(&db);
    Ok(SqliteConnection::establish(&db)?)
}

// Set the diesel `DATABASE_URL` key to the same thing as `BENCHER_DB`
fn diesel_database_url(db: &str) {
    if let Ok(database_url) = std::env::var(DATABASE_URL) {
        if database_url == db {
            return;
        }
        trace!(
            "\"{DATABASE_URL}\" ({database_url}) must be the same value as \"{BENCHER_DB}\" ({db})"
        );
    } else {
        trace!("Failed to find \"{DATABASE_URL}\"");
    }
    trace!("Setting \"{DATABASE_URL}\" to \"{BENCHER_DB}\" ({db})");
    std::env::set_var(DATABASE_URL, db)
}

fn run_migrations(conn: &mut SqliteConnection) -> Result<(), ApiError> {
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(ApiError::Migrations)?;
    Ok(())
}

fn get_config() -> ConfigDropshot {
    let port = std::env::var(PORT_KEY).unwrap_or_else(|_| DEFAULT_PORT.into());
    let address = format!("{DEFAULT_IP}:{port}");

    ConfigDropshot {
        bind_address: address.parse().unwrap(),
        request_body_max_bytes: MAX_BODY_SIZE,
        tls: None,
    }
}

// TODO set logging level the same as tracing
fn get_logger(api_name: &str) -> Result<slog::Logger, ApiError> {
    let config_logging = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    };
    config_logging
        .to_logger(api_name)
        .map_err(ApiError::CreateLogger)
}
