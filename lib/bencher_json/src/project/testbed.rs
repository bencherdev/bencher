use std::fmt;

use bencher_valid::{DateTime, ResourceName, Slug};
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ProjectUuid;

pub const TESTBED_LOCALHOST_STR: &str = "localhost";
#[allow(clippy::expect_used)]
static TESTBED_LOCALHOST: Lazy<ResourceName> = Lazy::new(|| {
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

crate::typed_uuid::typed_uuid!(TestbedUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewTestbed {
    pub name: ResourceName,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTestbeds(pub Vec<JsonTestbed>);

crate::from_vec!(JsonTestbeds[JsonTestbed]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTestbed {
    pub uuid: TestbedUuid,
    pub project: ProjectUuid,
    pub name: ResourceName,
    pub slug: Slug,
    pub created: DateTime,
    pub modified: DateTime,
}

impl fmt::Display for JsonTestbed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateTestbed {
    pub name: Option<ResourceName>,
    pub slug: Option<Slug>,
}
