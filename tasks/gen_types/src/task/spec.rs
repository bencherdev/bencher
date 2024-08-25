use std::fs;

use bencher_api::endpoints::Api;
use dropshot::{ApiDescription, EndpointTagPolicy, TagConfig, TagDetails};

use crate::{parser::TaskSpec, API_VERSION};

const SPEC_PATH: &str = "./services/api/openapi.json";

#[derive(Debug)]
pub struct Spec {}

impl TryFrom<TaskSpec> for Spec {
    type Error = anyhow::Error;

    fn try_from(_task: TaskSpec) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Spec {
    #[allow(clippy::unused_self)]
    pub fn exec(&self) -> anyhow::Result<()> {
        let _log = bencher_logger::bootstrap_logger();

        println!("🐰 Bencher OpenAPI Spec v{API_VERSION}",);

        println!("Generating OpenAPI Spec JSON file at: {SPEC_PATH}");
        let mut api_description = ApiDescription::new();
        // TODO add an argument to toggle whether to include the plus endpoints
        Api::register(
            &mut api_description,
            false,
            #[cfg(feature = "plus")]
            true,
        )
        .map_err(|e| anyhow::anyhow!("Failed to register API: {e}"))?;
        let mut spec_file = fs::File::create(SPEC_PATH)?;

        api_description.tag_config(TagConfig {
            allow_other_tags: false,
            policy: EndpointTagPolicy::AtLeastOne,
            tags: literally::hmap!{
                "auth" => TagDetails { description: Some("Auth".into()), external_docs: None},
                "organizations" => TagDetails { description: Some("Organizations".into()), external_docs: None},
                "projects" => TagDetails { description: Some("Projects".into()), external_docs: None},
                "reports" => TagDetails { description: Some("Reports".into()), external_docs: None},
                "perf" => TagDetails { description: Some("Perf Metrics".into()), external_docs: None},
                "plots" => TagDetails { description: Some("Plots".into()), external_docs: None},
                "branches" => TagDetails { description: Some("Branches".into()), external_docs: None},
                "testbeds" => TagDetails { description: Some("Testbeds".into()), external_docs: None},
                "benchmarks" => TagDetails { description: Some("Benchmarks".into()), external_docs: None},
                "measures" => TagDetails { description: Some("Measures".into()), external_docs: None},
                "metrics" => TagDetails { description: Some("Metrics".into()), external_docs: None},
                "thresholds" => TagDetails { description: Some("Thresholds".into()), external_docs: None},
                "models" => TagDetails { description: Some("Models".into()), external_docs: None},
                "alerts" => TagDetails { description: Some("Alerts".into()), external_docs: None},
                "users" => TagDetails { description: Some("Users".into()), external_docs: None},
                "tokens" => TagDetails { description: Some("API Tokens".into()), external_docs: None},
                "server" => TagDetails { description: Some("Server".into()), external_docs: None},
        }})
            .openapi(bencher_api::config::API_NAME, API_VERSION)
            .write(&mut spec_file)
            ?;

        println!("Saved OpenAPI JSON file to: {SPEC_PATH}");

        test_spec()?;
        fs::create_dir_all("./services/console/public/download")?;
        fs::copy(SPEC_PATH, "./services/console/public/download/openapi.json")?;

        Ok(())
    }
}

pub fn test_spec() -> anyhow::Result<()> {
    let spec_str = fs::read_to_string(SPEC_PATH)?;
    let spec: bencher_json::JsonSpec = serde_json::from_str(&spec_str)?;
    let version = spec
        .version()
        .ok_or_else(|| anyhow::anyhow!("No version found in openapi.json"))?;
    anyhow::ensure!(
        version == API_VERSION,
        "OpenAPI Spec version {version} does not match current version {API_VERSION}"
    );
    Ok(())
}
