use dropshot::{
    ApiDescription,
    ConfigDropshot,
    ConfigLogging,
    ConfigLoggingLevel,
    HttpServer,
    ServerContext,
};

const PORT_KEY: &str = "BENCHER_PORT";
const DEFAULT_IP: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "8080";

// TODO increase and add as a customizable feature
// 1 megabyte or 1_048_576 bytes
const MAX_BODY_SIZE: usize = 1 << 20;

use super::registrar::Registrar;

pub async fn get_server<Context>(
    api_name: &str,
    registrar: &mut impl Registrar<Context>,
    private: Context,
) -> Result<HttpServer<Context>, String>
where
    Context: ServerContext,
{
    let config = get_config();

    let mut api = ApiDescription::new();
    registrar.register(&mut api)?;

    let log = get_logger(api_name)?;

    Ok(
        dropshot::HttpServerStarter::new(&config, api, private, &log)
            .map_err(|error| format!("Failed to create server for {api_name}: {error}"))?
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

pub fn get_logger(api_name: &str) -> Result<slog::Logger, String> {
    let config_logging = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    };
    config_logging
        .to_logger(api_name)
        .map_err(|error| format!("Failed to create logger for {api_name}: {error}"))
}
