// This exists solely to export a `Uuid` type to Typescript that then aliases to `string`.
#[typeshare::typeshare]
#[allow(dead_code)]
pub struct Uuid(pub uuid::Uuid);

macro_rules! typed_uuid {
    ($name:ident) => {
        // https://github.com/diesel-rs/diesel/blob/master/diesel_tests/tests/custom_types.rs
        #[typeshare::typeshare]
        #[derive(
            Debug,
            Clone,
            Copy,
            Default,
            PartialEq,
            Eq,
            Hash,
            derive_more::Display,
            serde::Serialize,
            serde::Deserialize,
        )]
        #[cfg_attr(feature = "schema", derive(JsonSchema))]
        #[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
        #[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
        #[allow(unused_qualifications)]
        pub struct $name(uuid::Uuid);

        #[allow(unused_qualifications)]
        impl From<$name> for uuid::Uuid {
            fn from(uuid: $name) -> Self {
                uuid.0
            }
        }

        #[allow(unused_qualifications)]
        impl From<uuid::Uuid> for $name {
            fn from(uuid: uuid::Uuid) -> Self {
                Self(uuid)
            }
        }

        #[allow(unused_qualifications)]
        impl From<$name> for crate::ResourceId {
            fn from(uuid: $name) -> Self {
                uuid.0.into()
            }
        }

        #[allow(unused_qualifications)]
        impl From<$name> for crate::NameId {
            fn from(uuid: $name) -> Self {
                uuid.0.into()
            }
        }

        #[allow(unused_qualifications)]
        impl std::str::FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(uuid::Uuid::parse_str(s)?))
            }
        }

        #[allow(unused_qualifications)]
        impl AsRef<uuid::Uuid> for $name {
            fn as_ref(&self) -> &uuid::Uuid {
                &self.0
            }
        }

        impl $name {
            #[allow(unused_qualifications)]
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4())
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

pub(crate) use typed_uuid;
