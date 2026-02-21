use std::fmt;
use std::sync::LazyLock;

use bencher_valid::{DateTime, NameId, ResourceId, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ProjectUuid;
#[cfg(feature = "plus")]
use crate::{JsonSpec, SpecResourceId};

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

/// An testbed UUID or slug.
#[typeshare::typeshare]
pub type TestbedResourceId = ResourceId<TestbedUuid, TestbedSlug>;

/// A testbed UUID, slug, or name.
#[typeshare::typeshare]
pub type TestbedNameId = NameId<TestbedUuid, TestbedSlug, ResourceName>;

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
    /// The UUID or slug of the hardware spec for this testbed.
    #[cfg(feature = "plus")]
    pub spec: Option<SpecResourceId>,
}

impl JsonNewTestbed {
    pub fn localhost() -> Self {
        Self {
            name: TESTBED_LOCALHOST.clone(),
            slug: TESTBED_LOCALHOST_SLUG.clone(),
            #[cfg(feature = "plus")]
            spec: None,
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
    #[cfg(feature = "plus")]
    pub spec: Option<JsonSpec>,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl fmt::Display for JsonTestbed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum JsonUpdateTestbed {
    Patch(JsonTestbedPatch),
    Null(JsonTestbedPatchNull),
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTestbedPatch {
    /// The new name of the testbed.
    /// Maximum length is 64 characters.
    pub name: Option<ResourceName>,
    /// The preferred new slug for the testbed.
    /// Maximum length is 64 characters.
    pub slug: Option<TestbedSlug>,
    /// The UUID or slug of the hardware spec for this testbed.
    /// Set to `null` to remove the current spec.
    #[cfg(feature = "plus")]
    pub spec: Option<SpecResourceId>,
    /// Set whether the testbed is archived.
    pub archived: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTestbedPatchNull {
    pub name: Option<ResourceName>,
    pub slug: Option<TestbedSlug>,
    #[cfg(feature = "plus")]
    pub spec: (),
    pub archived: Option<bool>,
}

impl<'de> Deserialize<'de> for JsonUpdateTestbed {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const NAME_FIELD: &str = "name";
        const SLUG_FIELD: &str = "slug";
        const SPEC_FIELD: &str = "spec";
        const ARCHIVED_FIELD: &str = "archived";
        const FIELDS: &[&str] = &[NAME_FIELD, SLUG_FIELD, SPEC_FIELD, ARCHIVED_FIELD];

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Name,
            Slug,
            Spec,
            Archived,
        }

        struct UpdateTestbedVisitor;

        impl<'de> Visitor<'de> for UpdateTestbedVisitor {
            type Value = JsonUpdateTestbed;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("JsonUpdateTestbed")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut name = None;
                let mut slug = None;
                #[cfg(feature = "plus")]
                let mut spec: Option<Option<SpecResourceId>> = None;
                #[cfg(not(feature = "plus"))]
                let mut spec: Option<Option<serde::de::IgnoredAny>> = None;
                let mut archived = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field(NAME_FIELD));
                            }
                            name = Some(map.next_value()?);
                        },
                        Field::Slug => {
                            if slug.is_some() {
                                return Err(de::Error::duplicate_field(SLUG_FIELD));
                            }
                            slug = Some(map.next_value()?);
                        },
                        Field::Spec => {
                            if spec.is_some() {
                                return Err(de::Error::duplicate_field(SPEC_FIELD));
                            }
                            spec = Some(map.next_value()?);
                        },
                        Field::Archived => {
                            if archived.is_some() {
                                return Err(de::Error::duplicate_field(ARCHIVED_FIELD));
                            }
                            archived = Some(map.next_value()?);
                        },
                    }
                }

                #[cfg(feature = "plus")]
                {
                    Ok(match spec {
                        Some(Some(spec)) => Self::Value::Patch(JsonTestbedPatch {
                            name,
                            slug,
                            spec: Some(spec),
                            archived,
                        }),
                        Some(None) => Self::Value::Null(JsonTestbedPatchNull {
                            name,
                            slug,
                            spec: (),
                            archived,
                        }),
                        None => Self::Value::Patch(JsonTestbedPatch {
                            name,
                            slug,
                            spec: None,
                            archived,
                        }),
                    })
                }
                #[cfg(not(feature = "plus"))]
                {
                    let _ = spec;
                    Ok(Self::Value::Patch(JsonTestbedPatch {
                        name,
                        slug,
                        archived,
                    }))
                }
            }
        }

        deserializer.deserialize_struct("JsonUpdateTestbed", FIELDS, UpdateTestbedVisitor)
    }
}
