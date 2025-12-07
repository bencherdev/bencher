use std::{fmt, marker::PhantomData, str::FromStr};

use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize, Serialize,
    de::{self, Deserializer, Unexpected, Visitor},
};

use crate::ValidError;

#[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(untagged)]
pub enum NameId<U, S, N> {
    Uuid(U),
    Slug(S),
    Name(N),
}

#[cfg(feature = "schema")]
impl<U, S, N> JsonSchema for NameId<U, S, N>
where
    U: JsonSchema,
    S: JsonSchema,
    N: JsonSchema,
{
    fn schema_name() -> String {
        "NameId".to_owned()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("bencher_valid::name_id::NameId")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::schema::Schema {
        // Unfortunately, this seems to be required to have an untagged enum.
        // Otherwise, you get a runtime error: `can only flatten structs and maps (got a string)`
        // I believe this is a shortcoming of https://github.com/oxidecomputer/progenitor
        // For now, we just use the lowest common denominator's schema.
        N::json_schema(generator)
    }
}

impl<U, S, N> FromStr for NameId<U, S, N>
where
    U: FromStr,
    S: FromStr,
    N: FromStr,
{
    type Err = ValidError;

    fn from_str(name_id: &str) -> Result<Self, Self::Err> {
        if let Ok(uuid) = U::from_str(name_id) {
            Ok(Self::Uuid(uuid))
        } else if let Ok(slug) = S::from_str(name_id) {
            Ok(Self::Slug(slug))
        } else if let Ok(name) = N::from_str(name_id) {
            Ok(Self::Name(name))
        } else {
            Err(ValidError::NameId(name_id.to_owned()))
        }
    }
}

impl<'de, U, S, N> Deserialize<'de> for NameId<U, S, N>
where
    U: FromStr,
    S: FromStr,
    N: FromStr,
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

struct NameIdVisitor<U, S, N> {
    marker: PhantomData<(U, S, N)>,
}

impl<U, S, N> Visitor<'_> for NameIdVisitor<U, S, N>
where
    U: FromStr,
    S: FromStr,
    N: FromStr,
{
    type Value = NameId<U, S, N>;

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
    fn name_id_uuid() {
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
    fn name_id_slug() {
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
    fn name_id_name() {
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
