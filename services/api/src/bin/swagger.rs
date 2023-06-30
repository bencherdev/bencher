use std::fs::File;

use bencher_api::endpoints::Api;
use dropshot::{ApiDescription, EndpointTagPolicy, TagConfig, TagDetails};
use tracing::info;

#[derive(Debug, thiserror::Error)]
pub enum SwaggerError {
    #[error("Failed to set global default logger")]
    SetGlobalDefault(#[from] tracing::subscriber::SetGlobalDefaultError),
    #[error("Failed to create swagger file: {0}")]
    CreateFile(std::io::Error),
    #[error("Failed to register API: {0}")]
    RegisterApi(#[from] bencher_api::ApiError),
    #[error("Failed to create swagger file: {0}")]
    WriteFile(serde_json::Error),
}

fn main() -> Result<(), SwaggerError> {
    // Install global subscriber configured based on RUST_LOG envvar.
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;

    info!(
        "\u{1f430} Bencher OpenAPI Spec v{}",
        env!("CARGO_PKG_VERSION")
    );

    const API_VERSION: &str = env!("CARGO_PKG_VERSION");
    // This is run via a `pre-push` git hook
    // So if the `SWAGGER_PATH` below is ever updated
    // also update `./git/hooks/pre-push` accordingly.
    const SWAGGER_PATH: &str = "../ui/src/components/docs/api/swagger.json";

    info!("Generating OpenAPI JSON file at: {SWAGGER_PATH}");
    let mut api_description = ApiDescription::new();
    Api::register(&mut api_description, false)?;
    let mut swagger_file = File::create(SWAGGER_PATH).map_err(SwaggerError::CreateFile)?;

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
        .map_err(SwaggerError::WriteFile)?;

    info!("Saved OpenAPI JSON file to: {SWAGGER_PATH}");

    Ok(())
}
