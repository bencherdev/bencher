use std::fmt;

use bencher_valid::{NonEmpty, Slug, Url};
use chrono::{DateTime, Utc};
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};
use uuid::Uuid;

pub mod alert;
pub mod benchmark;
pub mod boundary;
pub mod branch;
pub mod metric;
pub mod metric_kind;
pub mod perf;
pub mod report;
pub mod testbed;
pub mod threshold;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewProject {
    pub name: NonEmpty,
    pub slug: Option<Slug>,
    pub url: Option<Url>,
    pub visibility: Option<JsonVisibility>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjects(pub Vec<JsonProject>);

crate::from_vec!(JsonProjects[JsonProject]);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProject {
    pub uuid: Uuid,
    pub organization: Uuid,
    pub name: NonEmpty,
    pub slug: Slug,
    pub url: Option<Url>,
    pub visibility: JsonVisibility,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

impl fmt::Display for JsonProject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

// Unfortunately, we have to use a complex, custom type and deserializer here.
// Due to some limitations in JSON Schema, we can't just use an `Option<Option<Url>>`.
// We need to be able to disambiguate between:
// - a missing `url` key
// - a `url` key with the value of `null`
// If we were writing our own client, we could do something like this:
// https://docs.rs/serde_with/latest/serde_with/rust/double_option/index.html
// However, we need `progenitor` to create a client that can accommodate both use cases.
// Just isolating the variants to the `url` field doesn't work either
// because `dropshot` doesn't like a flattened and untagged inner struct enum.
// So we are left with this solution, a top-level, untagged enum.
// In the future, avoid this by not having nullable fields in the API that need to be individually modified.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum JsonUpdateProject {
    Patch(JsonProjectPatch),
    Null(JsonProjectPatchNull),
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjectPatch {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub url: Option<Url>,
    pub visibility: Option<JsonVisibility>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjectPatchNull {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub url: (),
    pub visibility: Option<JsonVisibility>,
}

impl<'de> Deserialize<'de> for JsonUpdateProject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const NAME_FIELD: &str = "name";
        const SLUG_FIELD: &str = "slug";
        const URL_FIELD: &str = "url";
        const VISIBILITY_FIELD: &str = "visibility";
        const FIELDS: &[&str] = &[NAME_FIELD, SLUG_FIELD, URL_FIELD, VISIBILITY_FIELD];

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Name,
            Slug,
            Url,
            Visibility,
        }

        struct UpdateProjectVisitor;

        impl<'de> Visitor<'de> for UpdateProjectVisitor {
            type Value = JsonUpdateProject;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("JsonUpdateProject")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut name = None;
                let mut slug = None;
                let mut url = None;
                let mut visibility = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            if name.is_some() {
                                return Err(serde::de::Error::duplicate_field(NAME_FIELD));
                            }
                            name = Some(map.next_value()?);
                        },
                        Field::Slug => {
                            if slug.is_some() {
                                return Err(serde::de::Error::duplicate_field(SLUG_FIELD));
                            }
                            slug = Some(map.next_value()?);
                        },
                        Field::Url => {
                            if url.is_some() {
                                return Err(serde::de::Error::duplicate_field(URL_FIELD));
                            }
                            url = Some(map.next_value()?);
                        },
                        Field::Visibility => {
                            if visibility.is_some() {
                                return Err(serde::de::Error::duplicate_field(VISIBILITY_FIELD));
                            }
                            visibility = Some(map.next_value()?);
                        },
                    }
                }

                Ok(match url {
                    Some(Some(url)) => Self::Value::Patch(JsonProjectPatch {
                        name,
                        slug,
                        url: Some(url),
                        visibility,
                    }),
                    Some(None) => Self::Value::Null(JsonProjectPatchNull {
                        name,
                        slug,
                        url: (),
                        visibility,
                    }),
                    None => Self::Value::Patch(JsonProjectPatch {
                        name,
                        slug,
                        url: None,
                        visibility,
                    }),
                })
            }
        }

        deserializer.deserialize_struct("JsonUpdateProject", FIELDS, UpdateProjectVisitor)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonVisibility {
    #[default]
    Public,
    #[cfg(feature = "plus")]
    Private,
}

impl JsonVisibility {
    pub fn is_public(&self) -> bool {
        matches!(self, Self::Public)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Display)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonProjectPermission {
    #[display(fmt = "view")]
    View,
    #[display(fmt = "create")]
    Create,
    #[display(fmt = "edit")]
    Edit,
    #[display(fmt = "delete")]
    Delete,
    #[display(fmt = "manage")]
    Manage,
    #[display(fmt = "view_role")]
    ViewRole,
    #[display(fmt = "create_role")]
    CreateRole,
    #[display(fmt = "edit_role")]
    EditRole,
    #[display(fmt = "delete_role")]
    DeleteRole,
}
