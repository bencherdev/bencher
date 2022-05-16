use std::sync::Mutex;

use diesel::pg::PgConnection;
use dropshot::ApiDescription;
use dropshot::EndpointTagPolicy;
use dropshot::TagConfig;
use dropshot::TagDetails;

const API_NAME: &str = "Bencher";
const API_VERSION: &str = "0.1.0";

fn main() -> Result<(), String> {
    let mut api = ApiDescription::new();
    register(&mut api)?;

    api.tag_config(TagConfig {
        allow_other_tags: false,
        endpoint_tag_policy: EndpointTagPolicy::ExactlyOne,
        tag_definitions: literally::hmap!{
            "dba" => TagDetails { description: Some("Database operations".into()), external_docs: None},
            "report" => TagDetails { description: Some("Benchmark reports".into()), external_docs: None},
            "metrics" => TagDetails { description: Some("Benchmark metrics".into()), external_docs: None},
    }})
        .openapi(API_NAME, API_VERSION)
        .write(&mut std::io::stdout())
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn register(api: &mut ApiDescription<Mutex<PgConnection>>) -> Result<(), String> {
    api.register(fn_dba::api::put::api_put_dba_migrate)?;
    api.register(fn_reports::api::put::api_put_reports)?;
    api.register(fn_reports::api::get::api_get_metrics)?;
    Ok(())
}
