use std::{fmt, str::FromStr};

use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    de::{self, Deserializer, Unexpected, Visitor},
    Deserialize, Serialize,
};
use uuid::Uuid;

use crate::{non_empty::is_valid_non_empty, NonEmpty, Slug, ValidError};

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct NameId(String);

pub enum NameIdKind<T> {
    Uuid(Uuid),
    Slug(Slug),
    Name(T),
}

impl FromStr for NameId {
    type Err = ValidError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // A UUID always a valid slug
        // A slug is always non-empty
        // And a non-empty string is always a valid name ID
        if is_valid_non_empty(value) {
            Ok(Self(value.into()))
        } else {
            Err(ValidError::NameId(value.into()))
        }
    }
}

impl<T> TryFrom<&NameId> for NameIdKind<T>
where
    T: FromStr<Err = ValidError>,
{
    type Error = ValidError;

    fn try_from(name_id: &NameId) -> Result<Self, Self::Error> {
        if let Ok(uuid) = Uuid::from_str(name_id.as_ref()) {
            Ok(Self::Uuid(uuid))
        } else if let Ok(slug) = Slug::from_str(name_id.as_ref()) {
            Ok(Self::Slug(slug))
        } else if let Ok(name) = T::from_str(name_id.as_ref()) {
            Ok(Self::Name(name))
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

impl From<NonEmpty> for NameId {
    fn from(non_empty: NonEmpty) -> Self {
        Self(non_empty.into())
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

impl<T> fmt::Display for NameIdKind<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Uuid(uuid) => uuid.fmt(f),
            Self::Slug(slug) => slug.fmt(f),
            Self::Name(name) => name.fmt(f),
        }
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
