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
pub enum NameId<U, S, T> {
    Uuid(U),
    Slug(S),
    Name(T),
}

impl<U, S, T> FromStr for NameId<U, S, T>
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

impl<U, S, T> Display for NameId<U, S, T>
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

impl<U, S, T> Serialize for NameId<U, S, T>
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

impl<'de, U, S, T> Deserialize<'de> for NameId<U, S, T>
where
    U: FromStr,
    S: FromStr,
    T: FromStr,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(NameIdVisitor {
            marker: PhantomData,
        })
    }
}

struct NameIdVisitor<U, S, T> {
    marker: PhantomData<(U, S, T)>,
}

impl<U, S, T> Visitor<'_> for NameIdVisitor<U, S, T>
where
    U: FromStr,
    S: FromStr,
    T: FromStr,
{
    type Value = NameId<U, S, T>;

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

#[cfg(test)]
mod tests {
    use super::NameId;
    use crate::{BranchName, Slug};
    use uuid::Uuid;

    #[test]
    fn test_name_id_uuid() {
        const UUID: &str = "123e4567-e89b-12d3-a456-426614174000";
        let name_id: NameId<Uuid, Slug, BranchName> = UUID.parse().unwrap();
        assert_eq!(name_id, NameId::Uuid(Uuid::parse_str(UUID).unwrap()));

        let serialized = serde_json::to_string(&name_id).unwrap();
        assert_eq!(serialized, format!("\"{UUID}\""));
        let deserialized: NameId<Uuid, Slug, BranchName> =
            serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, name_id);
    }

    #[test]
    fn test_name_id_slug() {
        const SLUG: &str = "my-slug";
        let name_id: NameId<Uuid, Slug, BranchName> = SLUG.parse().unwrap();
        assert_eq!(name_id, NameId::Slug(SLUG.parse().unwrap()));

        let serialized = serde_json::to_string(&name_id).unwrap();
        assert_eq!(serialized, format!("\"{SLUG}\""));
        let deserialized: NameId<Uuid, Slug, BranchName> =
            serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, name_id);
    }

    #[test]
    fn test_name_id_name() {
        const NAME: &str = "my/branch";
        let name_id: NameId<Uuid, Slug, BranchName> = NAME.parse().unwrap();
        assert_eq!(name_id, NameId::Name(NAME.parse().unwrap()));

        let serialized = serde_json::to_string(&name_id).unwrap();
        assert_eq!(serialized, format!("\"{NAME}\""));
        let deserialized: NameId<Uuid, Slug, BranchName> =
            serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, name_id);
    }
}
