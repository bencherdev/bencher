use dropshot::ApiDescription;
use dropshot::HttpServerStarter;

use fn_reports::api::register;
use fn_reports::config::get_config;
use fn_reports::log::get_logger;

const API_NAME: &str = "reports";

#[tokio::main]
async fn main() -> Result<(), String> {
    let config = get_config();

    let mut api = ApiDescription::new();
    register(&mut api)?;

    let private = ();

    let log = get_logger(API_NAME)?;

    let server = HttpServerStarter::new(&config, api, private, &log)
        .map_err(|error| format!("Failed to create server for {API_NAME}: {error}"))?
        .start();

    server.await
}
