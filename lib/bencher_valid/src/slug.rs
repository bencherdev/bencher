use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::{benchmark_name::MAX_BENCHMARK_NAME_LEN, ValidError};

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct Slug(String);

impl FromStr for Slug {
    type Err = ValidError;

    fn from_str(slug: &str) -> Result<Self, Self::Err> {
        if is_valid_slug(slug) {
            Ok(Self(slug.into()))
        } else {
            Err(ValidError::Slug(slug.into()))
        }
    }
}

impl AsRef<str> for Slug {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Slug> for String {
    fn from(slug: Slug) -> Self {
        slug.0
    }
}

impl<'de> Deserialize<'de> for Slug {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(SlugVisitor)
    }
}

struct SlugVisitor;

impl Visitor<'_> for SlugVisitor {
    type Value = Slug;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid slug")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg(feature = "db")]
impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for Slug
where
    DB: diesel::backend::Backend,
    for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
        + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        out.set_value(self.to_string());
        Ok(diesel::serialize::IsNull::No)
    }
}

#[cfg(feature = "db")]
impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for Slug
where
    DB: diesel::backend::Backend,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self(String::from_sql(bytes)?.as_str().parse()?))
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_slug(slug: &str) -> bool {
    !slug.is_empty() && slug.len() <= MAX_BENCHMARK_NAME_LEN && slug == slug::slugify(slug)
}

impl Slug {
    pub const MAX: usize = MAX_BENCHMARK_NAME_LEN;

    #[cfg(feature = "full")]
    pub fn new(slug: &str) -> Self {
        let rand_suffix = rand::random::<u32>().to_string();
        let slug = slug::slugify(if slug.len() + 1 + rand_suffix.len() > Self::MAX {
            let mid = Self::MAX - (1 + rand_suffix.len());
            slug.split_at(mid).0
        } else {
            slug
        });
        Self(format!("{slug}-{rand_suffix}"))
    }
}

#[cfg(test)]
mod test {

    use super::is_valid_slug;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_slug() {
        assert_eq!(true, is_valid_slug("a-valid-slug"));
        assert_eq!(true, is_valid_slug("2nd-valid-slug"));
        assert_eq!(true, is_valid_slug("client-submit-serialize-deserialize-handle-client-submit-serialize-deserialize-handle-1996529012"));

        assert_eq!(false, is_valid_slug(""));
        assert_eq!(false, is_valid_slug(" a-valid-slug"));
        assert_eq!(false, is_valid_slug("a- valid-slug"));
        assert_eq!(false, is_valid_slug("a-valid-slug "));
        assert_eq!(false, is_valid_slug(" a-valid-slug "));
        assert_eq!(false, is_valid_slug("-a-valid-slug"));
        assert_eq!(false, is_valid_slug("a-valid-slug-"));
        assert_eq!(false, is_valid_slug("-a-valid-slug-"));
        assert_eq!(false, is_valid_slug("a--valid-slug"));
        assert_eq!(false, is_valid_slug("a-Valid-slug"));
    }
}
