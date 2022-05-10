use dropshot::ConfigLogging;
use dropshot::ConfigLoggingLevel;

pub fn get_logger(api_name: &str) -> Result<slog::Logger, String> {
    let config_logging = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    };
    config_logging
        .to_logger(api_name)
        .map_err(|error| format!("Failed to create logger for {api_name}: {error}"))
}
