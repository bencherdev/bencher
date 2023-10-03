use std::fs::File;

use bencher_api::{endpoints::Api, util::logger::bootstrap_logger, API_VERSION, SWAGGER_PATH};
use dropshot::{ApiDescription, EndpointTagPolicy, TagConfig, TagDetails};
use slog::info;

#[derive(Debug, thiserror::Error)]
pub enum SwaggerError {
    #[error("Failed to create swagger file: {0}")]
    CreateFile(std::io::Error),
    #[error("Failed to register API: {0}")]
    RegisterApi(#[from] bencher_api::ApiError),
    #[error("Failed to create swagger file: {0}")]
    WriteFile(serde_json::Error),
}

fn main() -> Result<(), SwaggerError> {
    let log = bootstrap_logger();

    info!(&log, "🐰 Bencher OpenAPI Spec v{API_VERSION}",);

    info!(&log, "Generating OpenAPI JSON file at: {SWAGGER_PATH}");
    let mut api_description = ApiDescription::new();
    Api::register(&mut api_description, false)?;
    let mut swagger_file = File::create(SWAGGER_PATH).map_err(SwaggerError::CreateFile)?;

    api_description.tag_config(TagConfig {
        allow_other_tags: false,
        endpoint_tag_policy: EndpointTagPolicy::AtLeastOne,
        tag_definitions: literally::hmap!{
            "auth" => TagDetails { description: Some("Auth".into()), external_docs: None},
            "organizations" => TagDetails { description: Some("Organizations".into()), external_docs: None},
            "projects" => TagDetails { description: Some("Projects".into()), external_docs: None},
            "perf" => TagDetails { description: Some("Perf Metrics".into()), external_docs: None},
            "reports" => TagDetails { description: Some("Reports".into()), external_docs: None},
            "metric kinds" => TagDetails { description: Some("Metric Kinds".into()), external_docs: None},
            "branches" => TagDetails { description: Some("Branches".into()), external_docs: None},
            "testbeds" => TagDetails { description: Some("Testbeds".into()), external_docs: None},
            "benchmarks" => TagDetails { description: Some("Benchmarks".into()), external_docs: None},
            "thresholds" => TagDetails { description: Some("Thresholds".into()), external_docs: None},
            "statistics" => TagDetails { description: Some("Statistics".into()), external_docs: None},
            "alerts" => TagDetails { description: Some("Alerts".into()), external_docs: None},
            "users" => TagDetails { description: Some("Users".into()), external_docs: None},
            "tokens" => TagDetails { description: Some("API Tokens".into()), external_docs: None},
            "server" => TagDetails { description: Some("Server".into()), external_docs: None},
            "spec" => TagDetails { description: Some("OpenAPI Spec".into()), external_docs: None},
    }})
        .openapi(bencher_api::config::API_NAME, API_VERSION)
        .write(&mut swagger_file)
        .map_err(SwaggerError::WriteFile)?;

    info!(&log, "Saved OpenAPI JSON file to: {SWAGGER_PATH}");

    Ok(())
}
