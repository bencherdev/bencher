use std::{fmt, str::FromStr};

use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    de::{self, Deserializer, Unexpected, Visitor},
    Deserialize, Serialize,
};
use uuid::Uuid;

#[derive(Debug, Display, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ResourceId {
    Uuid(Uuid),
    Slug(String),
}

impl FromStr for ResourceId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if let Ok(uuid) = Uuid::try_parse(value) {
            return Ok(ResourceId::Uuid(uuid));
        }
        let slug = slug::slugify(value);
        if value == slug {
            return Ok(ResourceId::Slug(slug));
        }
        Err("Failed to convert to resource ID".into())
    }
}

impl<'de> Deserialize<'de> for ResourceId {
    fn deserialize<D>(deserializer: D) -> Result<ResourceId, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ResourceIdVisitor)
    }
}

struct ResourceIdVisitor;

impl<'de> Visitor<'de> for ResourceIdVisitor {
    type Value = ResourceId;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a resource ID as a slug or UUID.")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        ResourceId::from_str(value).map_err(|_| E::invalid_value(Unexpected::Str(value), &self))
    }
}
