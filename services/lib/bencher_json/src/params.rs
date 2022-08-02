use std::fmt;

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    de::{
        self,
        Deserializer,
        Unexpected,
        Visitor,
    },
    Deserialize,
};
use uuid::Uuid;

#[derive(Debug)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ResourceId(pub String);

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
        if let Ok(uuid) = Uuid::try_parse(value) {
            return Ok(ResourceId(uuid.to_string()));
        }
        let slug = slug::slugify(value);
        if value == slug {
            return Ok(ResourceId(slug));
        }
        Err(E::invalid_value(Unexpected::Str(value), &self))
    }
}
