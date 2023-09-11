macro_rules! typed_id {
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
            FromSqlRow,
            AsExpression,
        )]
        #[diesel(sql_type = diesel::sql_types::Integer)]
        pub struct $name(i32);

        impl From<$name> for i32 {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for $name
        where
            DB: diesel::backend::Backend,
            i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
        {
            fn to_sql<'b>(
                &'b self,
                out: &mut diesel::serialize::Output<'b, '_, DB>,
            ) -> diesel::serialize::Result {
                self.0.to_sql(out)
            }
        }

        impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for $name
        where
            DB: diesel::backend::Backend,
            i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
        {
            fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
                Ok(Self(i32::from_sql(bytes)?))
            }
        }
    };
}

pub(crate) use typed_id;
