use dropshot::endpoint;
use dropshot::ApiDescription;
use dropshot::ConfigDropshot;
use dropshot::ConfigLogging;
use dropshot::ConfigLoggingLevel;
use dropshot::HttpError;
use dropshot::HttpResponseOk;
use dropshot::HttpResponseUpdatedNoContent;
use dropshot::HttpServerStarter;
use dropshot::RequestContext;
use dropshot::TypedBody;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;

const API_NAME: &str = "reports";
const API_VERSION: &str = "v0";
const PORT_KEY: &str = "PORT";
const DEFAULT_IP: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "8080";

#[tokio::main]
async fn main() -> Result<(), String> {
    let port = std::env::var(PORT_KEY).unwrap_or(DEFAULT_PORT.into());
    let address = format!("{DEFAULT_IP}:{port}");

    let config_dropshot = ConfigDropshot {
        bind_address: address.parse().unwrap(),
        request_body_max_bytes: 1024,
        tls: None,
    };

    let config_logging = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    };
    let log = config_logging
        .to_logger(API_NAME)
        .map_err(|error| format!("Failed to create logger for {API_NAME}: {error}"))?;

    let mut api = ApiDescription::new();
    api.register(example_api_get_counter).unwrap();
    api.register(example_api_put_counter).unwrap();

    #[cfg(feature = "schema")]
    api.openapi(API_NAME, API_VERSION)
        .write(&mut std::io::stdout())
        .map_err(|e| e.to_string())?;

    let server = HttpServerStarter::new(&config_dropshot, api, (), &log)
        .map_err(|error| format!("Failed to create server for {API_NAME}: {error}"))?
        .start();

    /*
     * Wait for the server to stop.  Note that there's not any code to shut down
     * this server, so we should never get past this point.
     */
    server.await
}

/*
 * HTTP API interface
 */

/**
 * `CounterValue` represents the value of the API's counter, either as the
 * response to a GET request to fetch the counter or as the body of a PUT
 * request to update the counter.
 */
#[derive(Deserialize, Serialize, JsonSchema)]
struct CounterValue {
    counter: u64,
}

/**
 * Fetch the current value of the counter.
 */
#[endpoint {
    method = GET,
    path = "/reports",
}]
async fn example_api_get_counter(
    _rqctx: Arc<RequestContext<()>>,
) -> Result<HttpResponseOk<CounterValue>, HttpError> {
    Ok(HttpResponseOk(CounterValue { counter: 0 }))
}

/**
 * Update the current value of the counter.  Note that the special value of 10
 * is not allowed (just to demonstrate how to generate an error).
 */
#[endpoint {
    method = PUT,
    path = "/counter",
}]
async fn example_api_put_counter(
    _rqctx: Arc<RequestContext<()>>,
    update: TypedBody<CounterValue>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let updated_value = update.into_inner();

    if updated_value.counter == 10 {
        Err(HttpError::for_bad_request(
            Some(String::from("BadInput")),
            format!("do not like the number {}", updated_value.counter),
        ))
    } else {
        Ok(HttpResponseUpdatedNoContent())
    }
}
