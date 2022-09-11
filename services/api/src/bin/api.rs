use bencher_api::{util::db::get_db_connection, ApiError};
use tracing::{info, trace};

const API_NAME: &str = "Bencher API";

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    // Install global subscriber configured based on RUST_LOG envvar.
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Bencher API Server v{}", env!("CARGO_PKG_VERSION"));
    run().await
}

// This is run via a `pre-push` git hook
// So if the `SWAGGER_PATH` below is ever updated
// also update `./git/hooks/pre-push` accordingly.
#[cfg(feature = "swagger")]
async fn run() -> Result<(), ApiError> {
    use std::fs::File;

    use bencher_api::{endpoints::Api, util::registrar::Registrar};
    use dropshot::{ApiDescription, EndpointTagPolicy, TagConfig, TagDetails};

    const API_VERSION: &str = env!("CARGO_PKG_VERSION");
    const SWAGGER_PATH: &str = "../ui/src/components/docs/api/swagger.json";

    trace!("Generating Swagger JSON file at: {SWAGGER_PATH}");
    let db_connection = get_db_connection()?;
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
            "projects" => TagDetails { description: Some("Projects".into()), external_docs: None},
            "reports" => TagDetails { description: Some("Reports".into()), external_docs: None},
            "branches" => TagDetails { description: Some("Branches".into()), external_docs: None},
            "testbeds" => TagDetails { description: Some("Testbeds".into()), external_docs: None},
            "thresholds" => TagDetails { description: Some("Thresholds".into()), external_docs: None},
            "perf" => TagDetails { description: Some("Benchmark Perf".into()), external_docs: None},
    }})
        .openapi(API_NAME, API_VERSION)
        .write(&mut swagger_file)
        .map_err(ApiError::WriteSwaggerFile)?;

    Ok(())
}

#[cfg(not(feature = "swagger"))]
async fn run() -> Result<(), ApiError> {
    use bencher_api::util::{migrate::run_migrations, server::get_server, ApiContext};
    use dotenvy::dotenv;
    use tokio::sync::Mutex;

    const BENCHER_SECRET: &str = "BENCHER_SECRET";

    trace!("Importing .env file");
    dotenv()?;

    let secret_key = std::env::var(BENCHER_SECRET).unwrap_or_else(|e| {
        info!("Failed to find \"{BENCHER_SECRET}\": {e}");
        let secret_key = uuid::Uuid::new_v4().to_string();
        info!("Generated temporary secret key: {secret_key}");
        secret_key
    });

    trace!("Connecting to database");
    let mut db_conn = get_db_connection()?;
    run_migrations(&mut db_conn)?;

    let context = Mutex::new(ApiContext {
        db: db_conn,
        key: secret_key,
    });

    trace!("Starting server");
    get_server(API_NAME, context)
        .await?
        .await
        .map_err(ApiError::RunServer)
}
