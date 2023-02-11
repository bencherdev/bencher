use bencher_api::ApiError;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    // Install global subscriber configured based on RUST_LOG envvar.
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;

    info!(
        "\u{1f430} Bencher API Server v{}",
        env!("CARGO_PKG_VERSION")
    );
    run().await
}

// This is run via a `pre-push` git hook
// So if the `SWAGGER_PATH` below is ever updated
// also update `./git/hooks/pre-push` accordingly.
#[cfg(feature = "swagger")]
#[allow(clippy::unused_async)]
async fn run() -> Result<(), ApiError> {
    use std::fs::File;
    use tracing::trace;

    use bencher_api::{endpoints::Api, util::registrar::Registrar};
    use dropshot::{ApiDescription, EndpointTagPolicy, TagConfig, TagDetails};

    const API_VERSION: &str = env!("CARGO_PKG_VERSION");
    const SWAGGER_PATH: &str = "../ui/src/components/docs/api/swagger.json";

    trace!("Generating Swagger JSON file at: {SWAGGER_PATH}");
    let mut api_description = ApiDescription::new();
    Api::register(&mut api_description)?;
    let mut swagger_file = File::create(SWAGGER_PATH).map_err(ApiError::CreateSwaggerFile)?;

    trace!("Creating API description");
    api_description.tag_config(TagConfig {
        allow_other_tags: false,
        endpoint_tag_policy: EndpointTagPolicy::AtLeastOne,
        tag_definitions: literally::hmap!{
            "ping" => TagDetails { description: Some("Ping".into()), external_docs: None},
            "auth" => TagDetails { description: Some("User Authentication".into()), external_docs: None},
            "organizations" => TagDetails { description: Some("Organizations".into()), external_docs: None},
            "invites" => TagDetails { description: Some("Organization Invitations".into()), external_docs: None},
            "projects" => TagDetails { description: Some("Projects".into()), external_docs: None},
            "reports" => TagDetails { description: Some("Reports".into()), external_docs: None},
            "branches" => TagDetails { description: Some("Branches".into()), external_docs: None},
            "testbeds" => TagDetails { description: Some("Testbeds".into()), external_docs: None},
            "thresholds" => TagDetails { description: Some("Thresholds".into()), external_docs: None},
            "perf" => TagDetails { description: Some("Benchmark Perf".into()), external_docs: None},
    }})
        .openapi(bencher_api::config::API_NAME, API_VERSION)
        .write(&mut swagger_file)
        .map_err(ApiError::WriteSwaggerFile)?;

    Ok(())
}

#[cfg(not(feature = "swagger"))]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::integer_arithmetic,
    clippy::integer_arithmetic
)]
async fn run() -> Result<(), ApiError> {
    use bencher_api::config::{config_tx::ConfigTx, Config};
    use dropshot::HttpServer;
    use tracing::error;

    loop {
        let config = Config::load_or_default().await?;
        let (restart_tx, mut restart_rx) = tokio::sync::mpsc::channel(1);
        let config_tx = ConfigTx { config, restart_tx };

        let handle = tokio::spawn(async move {
            async fn run_http_server(config_tx: ConfigTx) -> Result<(), ApiError> {
                HttpServer::try_from(config_tx)?
                    .await
                    .map_err(ApiError::RunServer)
            }

            if let Err(e) = run_http_server(config_tx).await {
                error!("Server Failure: {e}");
            }
        });

        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            restart = restart_rx.recv() => {
                if restart.is_some() {
                    handle.abort();
                } else {
                    break;
                }
            },
        }
    }

    Ok(())
}
