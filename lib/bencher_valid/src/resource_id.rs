use std::{fmt, str::FromStr};

use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    de::{self, Deserializer, Unexpected, Visitor},
    Deserialize, Serialize,
};
use uuid::Uuid;

use crate::{slug::is_valid_slug, Slug, ValidError};

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ResourceId(String);

pub enum ResourceIdKind {
    Uuid(Uuid),
    Slug(Slug),
}

impl FromStr for ResourceId {
    type Err = ValidError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // A UUID is always a valid slug
        // And a slug is always a valid resource ID
        if is_valid_slug(value) {
            Ok(Self(value.into()))
        } else {
            Err(ValidError::ResourceId(value.into()))
        }
    }
}

impl TryFrom<&ResourceId> for ResourceIdKind {
    type Error = ValidError;

    fn try_from(resource_id: &ResourceId) -> Result<Self, Self::Error> {
        if let Ok(uuid) = Uuid::from_str(resource_id.as_ref()) {
            Ok(Self::Uuid(uuid))
        } else if let Ok(slug) = Slug::from_str(resource_id.as_ref()) {
            Ok(Self::Slug(slug))
        } else {
            Err(ValidError::ResourceId(resource_id.as_ref().into()))
        }
    }
}

impl From<Uuid> for ResourceId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid.to_string())
    }
}

impl From<Slug> for ResourceId {
    fn from(slug: Slug) -> Self {
        Self(slug.into())
    }
}

impl AsRef<str> for ResourceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<ResourceId> for String {
    fn from(resource_id: ResourceId) -> Self {
        resource_id.0
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

impl Visitor<'_> for ResourceIdVisitor {
    type Value = ResourceId;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid UUID or slug.")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        ResourceId::from_str(value).map_err(|_e| E::invalid_value(Unexpected::Str(value), &self))
    }
}
