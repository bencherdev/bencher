pub mod db;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, diesel::FromSqlRow, diesel::AsExpression)]
#[diesel(sql_type = diesel::sql_types::BigInt)]
pub struct Rank(i64);
