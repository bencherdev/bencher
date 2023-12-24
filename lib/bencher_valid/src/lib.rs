use std::str::FromStr;

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

mod benchmark_name;
mod boundary;
mod branch_name;
mod date_time;
mod email;
mod error;
mod git_hash;
mod jwt;
mod name_id;
#[cfg(feature = "plus")]
mod plus;
mod resource_id;
mod resource_name;
mod sample_size;
mod secret;
mod slug;
mod url;
mod user_name;
mod window;

pub use crate::git_hash::GitHash;
pub use crate::slug::Slug;
pub use crate::url::Url;
pub use benchmark_name::BenchmarkName;
pub use boundary::Boundary;
pub use branch_name::BranchName;
pub use date_time::{DateTime, DateTimeMillis};
pub use email::Email;
pub use error::ValidError;
use error::REGEX_ERROR;
pub use jwt::Jwt;
pub use name_id::{NameId, NameIdKind};
#[cfg(feature = "plus")]
pub use plus::{
    CardBrand, CardCvc, CardNumber, Entitlements, ExpirationMonth, ExpirationYear, LastFour,
    LicensedPlanId, MeteredPlanId, PlanLevel, PlanStatus,
};
pub use resource_id::{ResourceId, ResourceIdKind};
pub use resource_name::ResourceName;
pub use sample_size::SampleSize;
pub use secret::Secret;
pub use user_name::UserName;
pub use window::Window;

const MAX_LEN: usize = 50;

#[cfg(feature = "wasm")]
#[wasm_bindgen(start)]
pub fn startup() {
    console_error_panic_hook::set_once();
}

fn is_valid_len(input: &str) -> bool {
    !input.is_empty() && input.len() <= MAX_LEN && input == input.trim()
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[cfg_attr(not(feature = "wasm"), allow(dead_code))]
pub fn is_valid_uuid(uuid: &str) -> bool {
    uuid::Uuid::from_str(uuid).is_ok()
}

pub trait Sanitize {
    fn sanitize(&mut self);
}

impl<T> Sanitize for Option<T>
where
    T: Sanitize,
{
    fn sanitize(&mut self) {
        if let Some(inner) = self {
            inner.sanitize();
        }
    }
}

#[cfg(feature = "db")]
macro_rules! typed_string {
    ($name:ident) => {
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
                String::from_sql(bytes)?
                    .as_str()
                    .parse()
                    .map_err(Into::into)
            }
        }
    };
}

#[cfg(feature = "db")]
pub(crate) use typed_string;

#[cfg(test)]
pub mod test {

    use super::is_valid_len;
    use pretty_assertions::assert_eq;

    pub const LEN_0_STR: &str = "";
    const LEN_1_STR: &str = "0";
    const LEN_2_STR: &str = "01";
    const LEN_3_STR: &str = "012";
    pub const LEN_50_STR: &str = "01234567890123456789012345678901234567890123456789";
    pub const LEN_51_STR: &str = "012345678901234567890123456789012345678901234567890";

    const PRE_SPACE_STR: &str = " 0123";
    const POST_SPACE_STR: &str = "0123 ";
    const BOTH_SPACE_STR: &str = " 0123 ";
    const INNER_SPACE_STR: &str = "01 23";

    #[test]
    fn test_is_valid_len() {
        assert_eq!(true, is_valid_len(LEN_1_STR));
        assert_eq!(true, is_valid_len(LEN_2_STR));
        assert_eq!(true, is_valid_len(LEN_3_STR));
        assert_eq!(true, is_valid_len(LEN_50_STR));
        assert_eq!(true, is_valid_len(INNER_SPACE_STR));

        assert_eq!(false, is_valid_len(LEN_0_STR));
        assert_eq!(false, is_valid_len(LEN_51_STR));
        assert_eq!(false, is_valid_len(PRE_SPACE_STR));
        assert_eq!(false, is_valid_len(POST_SPACE_STR));
        assert_eq!(false, is_valid_len(BOTH_SPACE_STR));
    }
}
