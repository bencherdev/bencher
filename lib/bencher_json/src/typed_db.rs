// https://github.com/diesel-rs/diesel/blob/master/diesel_tests/tests/custom_types.rs
macro_rules! typed_db {
    ($name:ident) => {
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

pub(crate) use typed_db;
