use bencher_valid::{NonEmpty, Slug};
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const TESTBED_LOCALHOST_STR: &str = "localhost";
#[allow(clippy::expect_used)]
static TESTBED_LOCALHOST: Lazy<NonEmpty> = Lazy::new(|| {
    TESTBED_LOCALHOST_STR
        .parse()
        .expect("Failed to parse testbed name.")
});
#[allow(clippy::expect_used)]
static TESTBED_LOCALHOST_SLUG: Lazy<Option<Slug>> = Lazy::new(|| {
    Some(
        TESTBED_LOCALHOST_STR
            .parse()
            .expect("Failed to parse testbed slug."),
    )
});

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewTestbed {
    pub name: NonEmpty,
    pub slug: Option<Slug>,
}

impl JsonNewTestbed {
    pub fn localhost() -> Self {
        Self {
            name: TESTBED_LOCALHOST.clone(),
            slug: TESTBED_LOCALHOST_SLUG.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(debug_assertions, derive(Default))]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTestbed {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: NonEmpty,
    pub slug: Slug,
}
