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
use uuid::Uuid;

use crate::{Slug, ValidError};

#[typeshare::typeshare]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct NameId<T>(T);

pub trait NamedUuid {
    fn uuid(&self) -> Uuid;
}

pub trait NamedSlug {
    fn slug(&self) -> Slug;
}

#[typeshare::typeshare]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum NamedId<U, S, T> {
    Uuid(U),
    Slug(S),
    Name(T),
}

impl<U, S, T> TryFrom<&NameId<T>> for NamedId<U, S, T>
where
    U: NamedUuid + FromStr<Err = uuid::Error>,
    S: NamedSlug + FromStr<Err = ValidError>,
    T: AsRef<str> + FromStr<Err = ValidError> + Clone + fmt::Debug + Send + Sync + 'static,
{
    type Error = ValidError;

    fn try_from(name_id: &NameId<T>) -> Result<Self, Self::Error> {
        name_id
            .as_ref()
            .parse()
            .map_err(|error| ValidError::FromNameId(Box::new(error)))
    }
}

impl<U, S, T> FromStr for NamedId<U, S, T>
where
    U: NamedUuid + FromStr<Err = uuid::Error>,
    S: NamedSlug + FromStr<Err = ValidError>,
    T: AsRef<str> + FromStr<Err = ValidError> + Clone + fmt::Debug + Send + Sync + 'static,
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
            Err(ValidError::FromNameId(Box::new(name_id.to_owned())))
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
    U: NamedUuid + FromStr<Err = uuid::Error>,
    S: NamedSlug + FromStr<Err = ValidError>,
    T: AsRef<str> + FromStr<Err = ValidError> + Clone + fmt::Debug + Send + Sync + 'static,
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
    U: NamedUuid + FromStr<Err = uuid::Error>,
    S: NamedSlug + FromStr<Err = ValidError>,
    T: AsRef<str> + FromStr<Err = ValidError> + Clone + fmt::Debug + Send + Sync + 'static,
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

pub enum NameIdKind<T> {
    Uuid(Uuid),
    Slug(Slug),
    Name(T),
}

impl<T> FromStr for NameId<T>
where
    T: FromStr<Err = ValidError>,
{
    type Err = ValidError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // This works as long as a valid UUID and a valid Slug are always a valid T.
        Ok(Self(value.parse().map_err(|error| ValidError::NameId {
            value: value.to_owned(),
            error: Box::new(error),
        })?))
    }
}

impl<T> TryFrom<&NameId<T>> for NameIdKind<T>
where
    T: AsRef<str> + FromStr<Err = ValidError> + Clone + fmt::Debug + Send + Sync + 'static,
{
    type Error = ValidError;

    fn try_from(name_id: &NameId<T>) -> Result<Self, Self::Error> {
        if let Ok(uuid) = Uuid::from_str(name_id.as_ref()) {
            Ok(Self::Uuid(uuid))
        } else if let Ok(slug) = Slug::from_str(name_id.as_ref()) {
            Ok(Self::Slug(slug))
        } else if let Ok(name) = T::from_str(name_id.as_ref()) {
            Ok(Self::Name(name))
        } else {
            Err(ValidError::FromNameId(Box::new(
                name_id.clone().into_inner(),
            )))
        }
    }
}

/// Returns the inner value.
impl<T> NameId<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl From<Uuid> for NameId<Uuid> {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<Slug> for NameId<Slug> {
    fn from(slug: Slug) -> Self {
        Self(slug)
    }
}

impl<T> AsRef<str> for NameId<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<T> From<NameId<T>> for String
where
    T: Display,
{
    fn from(name_id: NameId<T>) -> Self {
        name_id.0.to_string()
    }
}

impl<T> Display for NameId<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T> Serialize for NameId<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<T> Display for NameIdKind<T>
where
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

impl<'de, T> Deserialize<'de> for NameId<T>
where
    T: Deserialize<'de> + FromStr<Err = ValidError>,
{
    fn deserialize<D>(deserializer: D) -> Result<NameId<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(NameIdVisitor {
            marker: PhantomData,
        })
    }
}

struct NameIdVisitor<T> {
    marker: PhantomData<T>,
}

impl<T> Visitor<'_> for NameIdVisitor<T>
where
    T: FromStr<Err = ValidError>,
{
    type Value = NameId<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid UUID, slug, or name.")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        NameId::from_str(v).map_err(|_e| E::invalid_value(Unexpected::Str(v), &self))
    }
}
