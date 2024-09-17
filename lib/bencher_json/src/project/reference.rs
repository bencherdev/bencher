use bencher_valid::{DateTime, GitHash};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::BranchUuid;

crate::typed_uuid::typed_uuid!(ReferenceUuid);
crate::typed_uuid::typed_uuid!(VersionUuid);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReference {
    pub uuid: ReferenceUuid,
    pub branch: BranchUuid,
    pub start_point: Option<JsonStartPoint>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonStartPoint {
    pub branch: BranchUuid,
    pub reference: ReferenceUuid,
    pub version: JsonVersion,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonVersion {
    pub number: VersionNumber,
    pub hash: Option<GitHash>,
}

#[typeshare::typeshare]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, derive_more::Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
pub struct VersionNumber(pub u32);

#[cfg(feature = "db")]
mod version_number {
    use super::VersionNumber;

    impl VersionNumber {
        #[must_use]
        pub fn increment(self) -> Self {
            Self(self.0.checked_add(1).unwrap_or_default())
        }

        #[must_use]
        pub fn decrement(self) -> Self {
            Self(self.0.checked_sub(1).unwrap_or_default())
        }
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for VersionNumber
    where
        DB: diesel::backend::Backend,
        for<'a> i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(i32::try_from(self.0)?);
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for VersionNumber
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            Ok(Self(u32::try_from(i32::from_sql(bytes)?)?))
        }
    }
}
