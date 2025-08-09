use std::{
    fmt::{self, Display},
    marker::PhantomData,
    str::FromStr,
};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize, Serialize,
    de::{self, Deserializer, Unexpected, Visitor},
};

use crate::ValidError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema", schemars(untagged))]
pub enum NamedId<U, S, T> {
    Uuid(U),
    Slug(S),
    Name(T),
}

impl<U, S, T> FromStr for NamedId<U, S, T>
where
    U: FromStr,
    S: FromStr,
    T: FromStr,
{
    type Err = ValidError;

    fn from_str(name_id: &str) -> Result<Self, Self::Err> {
        if let Ok(uuid) = U::from_str(name_id) {
            Ok(Self::Uuid(uuid))
        } else if let Ok(slug) = S::from_str(name_id) {
            Ok(Self::Slug(slug))
        } else if let Ok(name) = T::from_str(name_id) {
            Ok(Self::Name(name))
        } else {
            Err(ValidError::NameId(name_id.to_owned()))
        }
    }
}

impl<U, S, T> Display for NamedId<U, S, T>
where
    U: Display,
    S: Display,
    T: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Uuid(uuid) => uuid.fmt(f),
            Self::Slug(slug) => slug.fmt(f),
            Self::Name(name) => name.fmt(f),
        }
    }
}

impl<U, S, T> Serialize for NamedId<U, S, T>
where
    U: Serialize,
    S: Serialize,
    T: Serialize,
{
    fn serialize<SER>(&self, serializer: SER) -> Result<SER::Ok, SER::Error>
    where
        SER: serde::Serializer,
    {
        match self {
            Self::Uuid(uuid) => uuid.serialize(serializer),
            Self::Slug(slug) => slug.serialize(serializer),
            Self::Name(name) => name.serialize(serializer),
        }
    }
}

impl<'de, U, S, T> Deserialize<'de> for NamedId<U, S, T>
where
    U: FromStr,
    S: FromStr,
    T: FromStr,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(NamedIdVisitor {
            marker: PhantomData,
        })
    }
}

struct NamedIdVisitor<U, S, T> {
    marker: PhantomData<(U, S, T)>,
}

impl<U, S, T> Visitor<'_> for NamedIdVisitor<U, S, T>
where
    U: FromStr,
    S: FromStr,
    T: FromStr,
{
    type Value = NamedId<U, S, T>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid UUID, slug, or name.")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        NamedId::from_str(v).map_err(|_e| E::invalid_value(Unexpected::Str(v), &self))
    }
}
