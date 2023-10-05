macro_rules! typed_uuid {
    ($name:ident) => {
        // https://github.com/diesel-rs/diesel/blob/master/diesel_tests/tests/custom_types.rs
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
            diesel::FromSqlRow,
            diesel::AsExpression,
        )]
        #[diesel(sql_type = diesel::sql_types::Text)]
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
        impl From<$name> for bencher_json::ResourceId {
            fn from(uuid: $name) -> Self {
                uuid.0.into()
            }
        }

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

        impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for $name
        where
            DB: diesel::backend::Backend,
            String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
        {
            fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
                Ok(Self(String::from_sql(bytes)?.as_str().parse()?))
            }
        }

        impl $name {
            #[allow(unused_qualifications)]
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4())
            }
        }
    };
}

pub(crate) use typed_uuid;
