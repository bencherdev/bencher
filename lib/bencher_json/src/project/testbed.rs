use bencher_valid::Slug;
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const TESTBED_LOCALHOST: &str = "localhost";
static TESTBED_LOCALHOST_SLUG: Lazy<Option<Slug>> = Lazy::new(|| TESTBED_LOCALHOST.parse().ok());

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewTestbed {
    pub name: String,
    pub slug: Option<Slug>,
}

impl JsonNewTestbed {
    pub fn localhost() -> Self {
        Self {
            name: TESTBED_LOCALHOST.into(),
            slug: TESTBED_LOCALHOST_SLUG.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTestbed {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: String,
    pub slug: Slug,
}
