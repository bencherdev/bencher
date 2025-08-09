macro_rules! typed_slug {
    ($name:ident) => {
        // https://github.com/diesel-rs/diesel/blob/master/diesel_tests/tests/custom_types.rs
        #[typeshare::typeshare]
        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            derive_more::Display,
            serde::Serialize,
            serde::Deserialize,
        )]
        #[cfg_attr(feature = "schema", derive(JsonSchema))]
        #[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
        #[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
        pub struct $name($crate::Slug);

        impl From<$name> for $crate::Slug {
            fn from(slug: $name) -> Self {
                slug.0
            }
        }

        impl From<$crate::Slug> for $name {
            fn from(slug: $crate::Slug) -> Self {
                Self(slug)
            }
        }

        impl From<$name> for crate::ResourceId {
            fn from(slug: $name) -> Self {
                slug.0.into()
            }
        }

        impl From<$name> for crate::NameId<$crate::Slug> {
            fn from(slug: $name) -> Self {
                slug.0.into()
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::ValidError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self($crate::Slug::from_str(s)?))
            }
        }

        impl AsRef<$crate::Slug> for $name {
            fn as_ref(&self) -> &$crate::Slug {
                &self.0
            }
        }

        #[cfg(feature = "db")]
        impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for $name
        where
            DB: diesel::backend::Backend,
            for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
                + Into<
                    <DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer,
                >,
        {
            fn to_sql<'b>(
                &'b self,
                out: &mut diesel::serialize::Output<'b, '_, DB>,
            ) -> diesel::serialize::Result {
                // https://docs.rs/diesel/latest/diesel/serialize/trait.ToSql.html#examples
                out.set_value(self.to_string());
                Ok(diesel::serialize::IsNull::No)
            }
        }

        #[cfg(feature = "db")]
        impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for $name
        where
            DB: diesel::backend::Backend,
            String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
        {
            fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
                Ok(Self(String::from_sql(bytes)?.as_str().parse()?))
            }
        }
    };
}

pub(crate) use typed_slug;
