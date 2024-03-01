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
    /// The name of the testbed.
    /// Maximum length is 64 characters.
    pub name: ResourceName,
    /// The preferred slug for the testbed.
    /// If not provided, the slug will be generated from the name.
    /// If the provided or generated slug is already in use, a unique slug will be generated.
    /// Maximum length is 64 characters.
    pub slug: Option<Slug>,
    /// If set to `true` and a testbed with the same name already exits,
    /// the existing testbed will be returned without an error.
    /// This is useful in cases where there may be a race condition to create a new testbed,
    /// such as multiple jobs in a CI/CD pipeline.
    pub soft: Option<bool>,
}

impl JsonNewTestbed {
    pub fn localhost() -> Self {
        Self {
            name: TESTBED_LOCALHOST.clone(),
            slug: TESTBED_LOCALHOST_SLUG.clone(),
            soft: None,
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
    /// The new name of the testbed.
    /// Maximum length is 64 characters.
    pub name: Option<ResourceName>,
    /// The preferred new slug for the testbed.
    /// Maximum length is 64 characters.
    pub slug: Option<Slug>,
}
