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

use crate::ValidError;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::BigInt))]
pub struct Window(u32);

impl TryFrom<u32> for Window {
    type Error = ValidError;

    fn try_from(window: u32) -> Result<Self, Self::Error> {
        is_valid_window(window)
            .then_some(Self(window))
            .ok_or(ValidError::Window(window))
    }
}

impl From<Window> for i64 {
    fn from(window: Window) -> Self {
        i64::from(window.0)
    }
}

impl From<Window> for u32 {
    fn from(window: Window) -> Self {
        window.0
    }
}

impl Window {
    pub const MIN: Self = Self(1);
    pub const DAY: Self = Self(60 * 60 * 24);
    pub const THIRTY: Self = Self(30 * Self::DAY.0);
    pub const SIXTY: Self = Self(60 * Self::DAY.0);
    pub const NINETY: Self = Self(90 * Self::DAY.0);
    pub const YEAR: Self = Self(365 * Self::DAY.0);
    pub const MAX: Self = Self(u32::MAX);
}

impl FromStr for Window {
    type Err = ValidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(u32::from_str(s).map_err(ValidError::WindowStr)?)
    }
}

impl<'de> Deserialize<'de> for Window {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u32(WindowVisitor)
    }
}

struct WindowVisitor;

impl Visitor<'_> for WindowVisitor {
    type Value = Window;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a statistical sample size greater than or equal to 2")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_u32(u32::try_from(value).map_err(E::custom)?)
    }

    fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.try_into().map_err(E::custom)
    }
}

#[cfg(feature = "db")]
mod db {
    use super::Window;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::BigInt, DB> for Window
    where
        DB: diesel::backend::Backend,
        for<'a> i64: diesel::serialize::ToSql<diesel::sql_types::BigInt, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(i64::from(*self));
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB> for Window
    where
        DB: diesel::backend::Backend,
        i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            u32::try_from(i64::from_sql(bytes)?)?
                .try_into()
                .map_err(Into::into)
        }
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_window(window: u32) -> bool {
    window > 0
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::{is_valid_window, Window};

    #[test]
    #[allow(clippy::excessive_precision)]
    fn test_boundary() {
        assert_eq!(true, is_valid_window(Window::MIN.into()));
        assert_eq!(true, is_valid_window(Window::THIRTY.into()));
        assert_eq!(true, is_valid_window(Window::SIXTY.into()));
        assert_eq!(true, is_valid_window(Window::NINETY.into()));
        assert_eq!(true, is_valid_window(Window::YEAR.into()));
        assert_eq!(true, is_valid_window(Window::MAX.into()));

        assert_eq!(false, is_valid_window(0));
    }
}
