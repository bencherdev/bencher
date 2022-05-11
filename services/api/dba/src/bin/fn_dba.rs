const API_NAME: &str = "dba";

#[tokio::main]
async fn main() -> Result<(), String> {
    let config = util::config::get_config();

    let mut api = dropshot::ApiDescription::new();
    fn_dba::api::register(&mut api)?;

    let private = ();

    let log = util::log::get_logger(API_NAME)?;

    let server = dropshot::HttpServerStarter::new(&config, api, private, &log)
        .map_err(|error| format!("Failed to create server for {API_NAME}: {error}"))?
        .start();

    server.await
}
