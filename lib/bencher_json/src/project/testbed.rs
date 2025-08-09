use std::fmt;
use std::sync::LazyLock;

use bencher_valid::{DateTime, NameId, NamedId, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ProjectUuid;

pub const TESTBED_LOCALHOST_STR: &str = "localhost";
#[expect(clippy::expect_used)]
pub static DEFAULT_TESTBED: LazyLock<TestbedNameId> = LazyLock::new(|| {
    TESTBED_LOCALHOST_STR
        .parse()
        .expect("Failed to parse testbed name.")
});
#[expect(clippy::expect_used)]
static TESTBED_LOCALHOST: LazyLock<ResourceName> = LazyLock::new(|| {
    TESTBED_LOCALHOST_STR
        .parse()
        .expect("Failed to parse testbed name.")
});
#[expect(clippy::expect_used)]
static TESTBED_LOCALHOST_SLUG: LazyLock<Option<TestbedSlug>> = LazyLock::new(|| {
    Some(
        TESTBED_LOCALHOST_STR
            .parse()
            .expect("Failed to parse testbed slug."),
    )
});

crate::typed_uuid::typed_uuid!(TestbedUuid);
crate::typed_slug::typed_slug!(TestbedSlug, ResourceName);

/// A testbed UUID, slug, or name.
pub type TestbedNameId = NamedId<TestbedUuid, TestbedSlug, ResourceName>;

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
    pub slug: Option<TestbedSlug>,
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
    pub slug: TestbedSlug,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
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
    pub slug: Option<TestbedSlug>,
    /// Set whether the testbed is archived.
    pub archived: Option<bool>,
}
