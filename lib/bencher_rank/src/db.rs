use crate::Rank;

impl<DB> diesel::serialize::ToSql<diesel::sql_types::BigInt, DB> for Rank
where
    DB: diesel::backend::Backend,
    i64: diesel::serialize::ToSql<diesel::sql_types::BigInt, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        self.0.to_sql(out)
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB> for Rank
where
    DB: diesel::backend::Backend,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        i64::from_sql(bytes).map(Self)
    }
}
