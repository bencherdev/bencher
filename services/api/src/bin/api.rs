use bencher_api::util::{
    db::get_db_connection,
    migrate::run_migration,
    server::get_server,
};
use tokio::sync::Mutex;

const API_NAME: &str = "Bencher API";
const BENCHER_SECRET_KEY: &str = "BENCHER_SECRET_KEY";

#[tokio::main]
async fn main() -> Result<(), String> {
    // install global subscriber configured based on RUST_LOG envvar.
    tracing_subscriber::fmt::init();
    tracing::info!("Bencher API Server v{}", env!("CARGO_PKG_VERSION"));
    run().await
}

#[cfg(feature = "swagger")]
async fn run() -> Result<(), String> {
    use std::fs::File;

    use bencher_api::{
        endpoints::Api,
        util::registrar::Registrar,
    };
    use dropshot::{
        ApiDescription,
        EndpointTagPolicy,
        TagConfig,
        TagDetails,
    };

    const API_VERSION: &str = env!("CARGO_PKG_VERSION");
    const SWAGGER_PATH: &str = "../ui/src/components/docs/api/swagger.json";

    let mut db_connection = get_db_connection().map_err(|e| e.to_string())?;
    let mut api_description = ApiDescription::new();
    Api::register(&mut api_description)?;
    let mut swagger_file = File::create(SWAGGER_PATH).map_err(|e| e.to_string())?;

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
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(not(feature = "swagger"))]
async fn run() -> Result<(), String> {
    use bencher_api::util::ApiContext;
    use dotenvy::dotenv;
    use tracing::info;

    dotenv().map_err(|e| e.to_string())?;

    let secret_key = std::env::var(BENCHER_SECRET_KEY).unwrap_or_else(|e| {
        info!("Failed to find \"{BENCHER_SECRET_KEY}\": {e}");
        let secret_key = uuid::Uuid::new_v4().to_string();
        info!("Generated temporary secret key: {secret_key}");
        secret_key
    });

    let mut db_conn = get_db_connection().map_err(|e| e.to_string())?;
    run_migration(&mut db_conn).map_err(|e| e.to_string())?;

    let context = Mutex::new(ApiContext {
        db:  db_conn,
        key: secret_key,
    });

    get_server(API_NAME, context).await?.await
}
