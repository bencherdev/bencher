use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::ValidError;

const V0_VERSION: u8 = 0;
const V1_VERSION: u8 = 1;
/// The accepted versions, spelled once for every message that has to name them.
pub const ACCEPTED_BMF_VERSIONS: &str = "0 or 1";

// One type serves both meanings of a BMF version: the version a report payload
// declares, and the version a results payload was parsed as. They are the same
// scale and they have to be able to differ, since a payload that declares version
// 1 may still hold v0 results, so they are two values of one type rather than two
// types. A project stores the highest version it accepts in the same type, and the
// versions are ordered because that gate is a maximum: a project accepts every
// version up to the one it names. That is internal to how this is used, and so is
// not part of the published description below, which says only what a client needs.
/// The Bencher Metric Format (BMF) version.
/// The accepted versions are 0 or 1.
/// If no version is specified, then version 0 is used.
#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
pub struct BmfVersion(u8);

impl TryFrom<u8> for BmfVersion {
    type Error = ValidError;

    fn try_from(version: u8) -> Result<Self, Self::Error> {
        is_valid_bmf_version(version)
            .then_some(Self(version))
            .ok_or(ValidError::BmfVersion(u64::from(version)))
    }
}

impl From<BmfVersion> for u8 {
    fn from(version: BmfVersion) -> Self {
        version.0
    }
}

impl From<BmfVersion> for i32 {
    fn from(version: BmfVersion) -> Self {
        Self::from(version.0)
    }
}

impl BmfVersion {
    /// A benchmark name maps to its measures.
    pub const V0: Self = Self(V0_VERSION);
    /// A benchmark name maps to an array of parameter set entries.
    pub const V1: Self = Self(V1_VERSION);
}

impl FromStr for BmfVersion {
    type Err = ValidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(u8::from_str(s).map_err(ValidError::BmfVersionStr)?)
    }
}

impl<'de> Deserialize<'de> for BmfVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u8(BmfVersionVisitor)
    }
}

struct BmfVersionVisitor;

impl Visitor<'_> for BmfVersionVisitor {
    type Value = BmfVersion;

    /// Names the accepted versions, because this is the message serde reports for
    /// a `bmf_version` that is not an unsigned integer at all, such as a string or
    /// a negative number.
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a BMF version, one of {ACCEPTED_BMF_VERSIONS}")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        // Reported as the value the payload carried, not as a truncation of it.
        let version = u8::try_from(v).map_err(|_e| E::custom(ValidError::BmfVersion(v)))?;
        self.visit_u8(version)
    }

    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.try_into().map_err(E::custom)
    }
}

#[cfg(feature = "db")]
mod db {
    use super::BmfVersion;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for BmfVersion
    where
        DB: diesel::backend::Backend,
        for<'a> i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(i32::from(*self));
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for BmfVersion
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            u8::try_from(i32::from_sql(bytes)?)?
                .try_into()
                .map_err(Into::into)
        }
    }
}

pub fn is_valid_bmf_version(version: u8) -> bool {
    matches!(version, V0_VERSION | V1_VERSION)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{ACCEPTED_BMF_VERSIONS, BmfVersion, is_valid_bmf_version};

    #[test]
    fn is_valid_bmf_version_true() {
        assert_eq!(true, is_valid_bmf_version(BmfVersion::V0.into()));
        assert_eq!(true, is_valid_bmf_version(BmfVersion::V1.into()));
    }

    #[test]
    fn is_valid_bmf_version_false() {
        for version in [2, 3, 10, u8::MAX] {
            assert_eq!(false, is_valid_bmf_version(version), "{version}");
        }
    }

    #[test]
    fn bmf_version_default_is_v0() {
        assert_eq!(BmfVersion::default(), BmfVersion::V0);
    }

    #[test]
    fn bmf_version_serializes_as_an_integer() {
        assert_eq!(serde_json::to_string(&BmfVersion::V0).unwrap(), "0");
        assert_eq!(serde_json::to_string(&BmfVersion::V1).unwrap(), "1");
    }

    #[test]
    fn bmf_version_deserializes_the_accepted_versions() {
        assert_eq!(
            serde_json::from_str::<BmfVersion>("0").unwrap(),
            BmfVersion::V0
        );
        assert_eq!(
            serde_json::from_str::<BmfVersion>("1").unwrap(),
            BmfVersion::V1
        );
    }

    /// Every rejected shape names the accepted versions.
    ///
    /// An out of range integer is reported through the validation error and
    /// anything that is not an unsigned integer at all is reported through the
    /// visitor, so both messages have to carry the list.
    #[test]
    fn bmf_version_rejection_names_the_accepted_versions() {
        for input in ["2", "255", "256", "-1", "\"1\"", "1.5", "true", "null"] {
            let error = serde_json::from_str::<BmfVersion>(input)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains(ACCEPTED_BMF_VERSIONS),
                "expected the rejection of {input} to name the accepted versions: {error}"
            );
        }
    }

    /// A project gate is a maximum, so the versions have to compare.
    #[test]
    fn bmf_version_is_ordered() {
        assert_eq!(true, BmfVersion::V0 < BmfVersion::V1);
        assert_eq!(BmfVersion::V0.max(BmfVersion::V1), BmfVersion::V1);
    }

    #[test]
    fn bmf_version_from_str() {
        assert_eq!("0".parse::<BmfVersion>().unwrap(), BmfVersion::V0);
        assert_eq!("1".parse::<BmfVersion>().unwrap(), BmfVersion::V1);
        "2".parse::<BmfVersion>().unwrap_err();
        "one".parse::<BmfVersion>().unwrap_err();
    }
}
