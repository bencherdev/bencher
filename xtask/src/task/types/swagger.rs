use std::fs::File;

use crate::parser::TaskSwagger;
use bencher_api::{endpoints::Api, API_VERSION};
use dropshot::{ApiDescription, EndpointTagPolicy, TagConfig, TagDetails};

const SWAGGER_PATH: &str = "./services/api/swagger.json";

#[derive(Debug)]
pub struct Swagger {}

impl TryFrom<TaskSwagger> for Swagger {
    type Error = anyhow::Error;

    fn try_from(_swagger: TaskSwagger) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Swagger {
    pub fn exec(&self) -> anyhow::Result<()> {
        let _log = bencher_logger::bootstrap_logger();

        println!("🐰 Bencher OpenAPI Spec v{API_VERSION}",);

        println!("Generating OpenAPI JSON file at: {SWAGGER_PATH}");
        let mut api_description = ApiDescription::new();
        // TODO add an argument to toggle whether to include the plus endpoints
        Api::register(
            &mut api_description,
            false,
            #[cfg(feature = "plus")]
            true,
        )
        .map_err(|e| anyhow::anyhow!("Failed to register API: {e}"))?;
        let mut swagger_file = File::create(SWAGGER_PATH)?;

        api_description.tag_config(TagConfig {
            allow_other_tags: false,
            endpoint_tag_policy: EndpointTagPolicy::AtLeastOne,
            tag_definitions: literally::hmap!{
                "auth" => TagDetails { description: Some("Auth".into()), external_docs: None},
                "organizations" => TagDetails { description: Some("Organizations".into()), external_docs: None},
                "projects" => TagDetails { description: Some("Projects".into()), external_docs: None},
                "reports" => TagDetails { description: Some("Reports".into()), external_docs: None},
                "perf" => TagDetails { description: Some("Perf Metrics".into()), external_docs: None},
                "branches" => TagDetails { description: Some("Branches".into()), external_docs: None},
                "testbeds" => TagDetails { description: Some("Testbeds".into()), external_docs: None},
                "benchmarks" => TagDetails { description: Some("Benchmarks".into()), external_docs: None},
                "measures" => TagDetails { description: Some("Measures".into()), external_docs: None},
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
            ?;

        println!("Saved OpenAPI JSON file to: {SWAGGER_PATH}");

        swagger_spec()?;

        Ok(())
    }
}

pub fn swagger_spec() -> anyhow::Result<bencher_json::JsonSpec> {
    let swagger_spec_str = std::fs::read_to_string(SWAGGER_PATH)?;
    serde_json::from_str(&swagger_spec_str).map_err(Into::into)
}
