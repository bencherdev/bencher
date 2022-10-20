#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConfig {
    #[serde(default = "default_url")]
    url: Url,
}

fn default_url() -> Url {
    let default = {
        #[cfg(debug_assertions)]
        {
            "http://localhost:3000"
        }
        #[cfg(not(debug_assertions))]
        {
            "https://bencher.dev"
        }
    };

    default
        .parse()
        .expect(&format!("Invalid default URL: {default}"))
}
