use dropshot::{ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, HttpServer};

const PORT_KEY: &str = "BENCHER_PORT";
const DEFAULT_IP: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "8080";

// TODO increase and add as a customizable feature
// 1 megabyte or 1_048_576 bytes
const MAX_BODY_SIZE: usize = 1 << 20;

use super::{registrar::Registrar, Context};
use crate::{endpoints::Api, ApiError};

pub async fn get_server(api_name: &str, private: Context) -> Result<HttpServer<Context>, ApiError> {
    let config = get_config();

    let mut api = ApiDescription::new();
    Api::register(&mut api)?;

    let log = get_logger(api_name)?;

    Ok(
        dropshot::HttpServerStarter::new(&config, api, private, &log)
            .map_err(ApiError::CreateServer)?
            .start(),
    )
}

pub fn get_config() -> ConfigDropshot {
    let port = std::env::var(PORT_KEY).unwrap_or(DEFAULT_PORT.into());
    let address = format!("{DEFAULT_IP}:{port}");

    ConfigDropshot {
        bind_address: address.parse().unwrap(),
        request_body_max_bytes: MAX_BODY_SIZE,
        tls: None,
    }
}

// TODO set logging level the same as tracing
pub fn get_logger(api_name: &str) -> Result<slog::Logger, ApiError> {
    let config_logging = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    };
    config_logging
        .to_logger(api_name)
        .map_err(ApiError::CreateLogger)
}
