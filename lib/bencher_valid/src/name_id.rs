use std::{fmt, str::FromStr};

use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    de::{self, Deserializer, Unexpected, Visitor},
    Deserialize, Serialize,
};
use uuid::Uuid;

use crate::{ResourceName, Slug, ValidError};

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct NameId(String);

pub enum NameIdKind {
    Uuid(Uuid),
    Slug(Slug),
    Name(ResourceName),
}

impl FromStr for NameId {
    type Err = ValidError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if let Ok(uuid) = Uuid::from_str(value) {
            Ok(Self(uuid.to_string()))
        } else if let Ok(slug) = Slug::from_str(value) {
            Ok(Self(slug.into()))
        } else if let Ok(resource_name) = ResourceName::from_str(value) {
            Ok(Self(resource_name.into()))
        } else {
            Err(ValidError::NameId(value.into()))
        }
    }
}

impl TryFrom<&NameId> for NameIdKind {
    type Error = ValidError;

    fn try_from(name_id: &NameId) -> Result<Self, Self::Error> {
        if let Ok(uuid) = Uuid::from_str(name_id.as_ref()) {
            Ok(Self::Uuid(uuid))
        } else if let Ok(slug) = Slug::from_str(name_id.as_ref()) {
            Ok(Self::Slug(slug))
        } else if let Ok(resource_name) = ResourceName::from_str(name_id.as_ref()) {
            Ok(Self::Name(resource_name))
        } else {
            Err(ValidError::NameId(name_id.to_string()))
        }
    }
}

impl From<Uuid> for NameId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid.to_string())
    }
}

impl From<Slug> for NameId {
    fn from(slug: Slug) -> Self {
        Self(slug.into())
    }
}

impl From<ResourceName> for NameId {
    fn from(resource_name: ResourceName) -> Self {
        Self(resource_name.into())
    }
}

impl AsRef<str> for NameId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<NameId> for String {
    fn from(name_id: NameId) -> Self {
        name_id.0
    }
}

impl<'de> Deserialize<'de> for NameId {
    fn deserialize<D>(deserializer: D) -> Result<NameId, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(NameIdVisitor)
    }
}

struct NameIdVisitor;

impl Visitor<'_> for NameIdVisitor {
    type Value = NameId;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid UUID or slug.")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        NameId::from_str(value).map_err(|_e| E::invalid_value(Unexpected::Str(value), &self))
    }
}
