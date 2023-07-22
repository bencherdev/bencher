use std::fmt;

use bencher_valid::{NonEmpty, Slug, Url};
use chrono::{DateTime, Utc};
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

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum JsonUpdateProject {
    Patch(JsonProjectPatch),
    Url(JsonProjectPatchUrl),
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjectPatch {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    #[serde(skip)]
    pub url: (),
    pub visibility: Option<JsonVisibility>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjectPatchUrl {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub url: Option<Url>,
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

        // // This part could also be generated independently by:
        // //
        // //    #[derive(Deserialize)]
        // //    #[serde(field_identifier, rename_all = "lowercase")]
        // //    enum Field { Secs, Nanos }
        // impl<'de> Deserialize<'de> for Field {
        //     fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
        //     where
        //         D: Deserializer<'de>,
        //     {
        //         struct FieldVisitor;

        //         impl<'de> Visitor<'de> for FieldVisitor {
        //             type Value = Field;

        //             fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        //                 formatter.write_str("`name`, `slug`, `url`, or `visibility`")
        //             }

        //             fn visit_str<E>(self, value: &str) -> Result<Field, E>
        //             where
        //                 E: serde::de::Error,
        //             {
        //                 match value {
        //                     NAME_FIELD => Ok(Field::Name),
        //                     SLUG_FIELD => Ok(Field::Slug),
        //                     URL_FIELD => Ok(Field::Url),
        //                     VISIBILITY_FIELD => Ok(Field::Visibility),
        //                     _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
        //                 }
        //             }
        //         }

        //         deserializer.deserialize_identifier(FieldVisitor)
        //     }
        // }

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

                Ok(if let Some(url) = url {
                    Self::Value::Url(JsonProjectPatchUrl {
                        name,
                        slug,
                        url,
                        visibility,
                    })
                } else {
                    Self::Value::Patch(JsonProjectPatch {
                        name,
                        slug,
                        url: (),
                        visibility,
                    })
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
