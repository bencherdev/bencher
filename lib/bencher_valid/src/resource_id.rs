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
pub enum ResourceId<U, S> {
    Uuid(U),
    Slug(S),
}

impl<U, S> From<S> for ResourceId<U, S> {
    fn from(slug: S) -> Self {
        Self::Slug(slug)
    }
}

#[cfg(feature = "schema")]
impl<U, S> JsonSchema for ResourceId<U, S>
where
    U: JsonSchema,
    S: JsonSchema,
{
    fn schema_name() -> String {
        "ResourceId".to_owned()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("bencher_valid::resource_id::ResourceId")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::schema::Schema {
        // Unfortunately, this seems to be required to have an untagged enum.
        // Otherwise, you get a runtime error: `can only flatten structs and maps (got a string)`
        // I believe this is a shortcoming of https://github.com/oxidecomputer/progenitor
        // For now, we just use the lowest common denominator's schema.
        S::json_schema(generator)
    }
}

impl<U, S> FromStr for ResourceId<U, S>
where
    U: FromStr,
    S: FromStr,
{
    type Err = ValidError;

    fn from_str(name_id: &str) -> Result<Self, Self::Err> {
        if let Ok(uuid) = U::from_str(name_id) {
            Ok(Self::Uuid(uuid))
        } else if let Ok(slug) = S::from_str(name_id) {
            Ok(Self::Slug(slug))
        } else {
            Err(ValidError::ResourceId(name_id.to_owned()))
        }
    }
}

impl<'de, U, S> Deserialize<'de> for ResourceId<U, S>
where
    U: FromStr,
    S: FromStr,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ResourceIdVisitor {
            marker: PhantomData,
        })
    }
}

struct ResourceIdVisitor<U, S> {
    marker: PhantomData<(U, S)>,
}

impl<U, S> Visitor<'_> for ResourceIdVisitor<U, S>
where
    U: FromStr,
    S: FromStr,
{
    type Value = ResourceId<U, S>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid UUID or slug.")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        ResourceId::from_str(v).map_err(|_e| E::invalid_value(Unexpected::Str(v), &self))
    }
}

#[cfg(test)]
mod tests {
    use super::ResourceId;
    use crate::Slug;
    use uuid::Uuid;

    #[test]
    fn test_resource_id_uuid() {
        const UUID: &str = "123e4567-e89b-12d3-a456-426614174000";
        let resource_id: ResourceId<Uuid, Slug> = UUID.parse().unwrap();
        assert_eq!(
            resource_id,
            ResourceId::Uuid(Uuid::parse_str(UUID).unwrap())
        );

        let serialized = serde_json::to_string(&resource_id).unwrap();
        assert_eq!(serialized, format!("\"{UUID}\""));
        let deserialized: ResourceId<Uuid, Slug> = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, resource_id);
    }

    #[test]
    fn test_resource_id_slug() {
        const SLUG: &str = "my-slug";
        let resource_id: ResourceId<Uuid, Slug> = SLUG.parse().unwrap();
        assert_eq!(resource_id, ResourceId::Slug(SLUG.parse().unwrap()));

        let serialized = serde_json::to_string(&resource_id).unwrap();
        assert_eq!(serialized, format!("\"{SLUG}\""));
        let deserialized: ResourceId<Uuid, Slug> = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, resource_id);
    }
}
