#[cfg(debug_assertions)]
use std::sync::LazyLock;

use base64::{
    Engine as _,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::ValidError;

#[cfg(debug_assertions)]
// Valid until 2159-12-06T18:53:44Z
const TEST_ADMIN_BENCHER_API_TOKEN_STR: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjQzNjA5LCJpYXQiOjE2OTg2NzYzMTQsImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJldXN0YWNlLmJhZ2dlQG5vd2hlcmUuY29tIiwib3JnIjpudWxsfQ.xumYID-R4waqhyjhcbSlwartbiRJ2AwngVkevLUBVCA";

#[cfg(debug_assertions)]
// Valid until 2159-12-06T18:53:44Z
const TEST_BENCHER_API_TOKEN_STR: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjM2MDI0LCJpYXQiOjE2OTg2Njg3MjksImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJtdXJpZWwuYmFnZ2VAbm93aGVyZS5jb20iLCJvcmciOm51bGx9.t3t23mlgKYZmUt7-PbRWLqXlCTt6Ydh8TRE8KiSGQi4";

#[cfg(debug_assertions)]
#[expect(clippy::expect_used)]
static TEST_ADMIN_BENCHER_API_TOKEN: LazyLock<Jwt> = LazyLock::new(|| {
    TEST_ADMIN_BENCHER_API_TOKEN_STR
        .parse()
        .expect("Invalid test JWT")
});

#[cfg(debug_assertions)]
#[expect(clippy::expect_used)]
static TEST_BENCHER_API_TOKEN: LazyLock<Jwt> = LazyLock::new(|| {
    TEST_BENCHER_API_TOKEN_STR
        .parse()
        .expect("Invalid test JWT")
});

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct Jwt(String);

#[cfg(feature = "db")]
crate::typed_string!(Jwt);

impl TryFrom<String> for Jwt {
    type Error = ValidError;

    fn try_from(jwt: String) -> Result<Self, Self::Error> {
        if is_valid_jwt(&jwt) {
            Ok(Self(jwt))
        } else {
            Err(ValidError::Jwt(jwt))
        }
    }
}

impl FromStr for Jwt {
    type Err = ValidError;

    fn from_str(jwt: &str) -> Result<Self, Self::Err> {
        Self::try_from(jwt.to_owned())
    }
}

impl AsRef<str> for Jwt {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Jwt> for String {
    fn from(jwt: Jwt) -> Self {
        jwt.0
    }
}

#[cfg(debug_assertions)]
impl Jwt {
    /// Create a valid admin test token
    pub fn test_admin_token() -> Self {
        TEST_ADMIN_BENCHER_API_TOKEN.clone()
    }

    /// Create a valid test token
    pub fn test_token() -> Self {
        TEST_BENCHER_API_TOKEN.clone()
    }
}

/// Takes the result of a rsplit and ensure we only get 2 parts
/// with a length greater than zero. Otherwise, return false.
macro_rules! expect_two {
    ($iter:expr) => {{
        let mut i = $iter;
        match (i.next(), i.next(), i.next()) {
            (Some(first), Some(second), None) if !first.is_empty() && !second.is_empty() => {
                (first, second)
            },
            _ => return false,
        }
    }};
}

// Based on
// https://github.com/validatorjs/validator.js/blob/63b61629187a732c3b3c8d89fe4cacad890cad99/src/lib/isJWT.js
// https://github.com/Keats/jsonwebtoken/blob/v8.1.1/src/decoding.rs#L167
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_jwt(jwt: &str) -> bool {
    let (signature, message) = expect_two!(jwt.rsplitn(2, '.'));
    let (payload, header) = expect_two!(message.rsplitn(2, '.'));

    // A URL safe encoding that does not have trailing `=` characters
    let url_safe = GeneralPurpose::new(
        &base64::alphabet::URL_SAFE,
        GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::RequireNone),
    );

    url_safe.decode(header).is_ok()
        && url_safe.decode(payload).is_ok()
        && url_safe.decode(signature).is_ok()
}

#[cfg(test)]
mod tests {
    use crate::Jwt;

    use super::is_valid_jwt;
    use pretty_assertions::assert_eq;

    const HEADER: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9";
    const PAYLOAD: &str = "eyJhdWQiOiJhdXRoIiwiZXhwIjoxNjY5Mjk5NjExLCJpYXQiOjE2NjkyOTc4MTEsImlzcyI6ImJlbmNoZXIuZGV2Iiwic3ViIjoiYUBhLmNvIiwib3JnIjpudWxsfQ";
    const SIGNATURE: &str = "jJmb_nCVJYLD5InaIxsQfS7x87fUsnCYpQK9SrWrKTc";

    #[test]
    fn is_valid_jwt_true() {
        assert_eq!(
            true,
            is_valid_jwt(&format!("{HEADER}.{PAYLOAD}.{SIGNATURE}"))
        );
    }

    #[test]
    fn is_valid_jwt_false() {
        for jwt in [
            "",
            &format!(".{PAYLOAD}.{SIGNATURE}"),
            &format!("{HEADER}..{SIGNATURE}"),
            &format!("{HEADER}.{PAYLOAD}."),
            &format!("{HEADER}.."),
            &format!(".{PAYLOAD}."),
            &format!("..{SIGNATURE}"),
            &format!(" {HEADER}.{PAYLOAD}.{SIGNATURE}"),
            &format!("{HEADER}.{PAYLOAD}.{SIGNATURE} "),
            &format!("{HEADER}.{PAYLOAD}"),
            &format!("{HEADER}."),
            &format!("{PAYLOAD}.{SIGNATURE}"),
            &format!(".{SIGNATURE}"),
            &format!("bad.{PAYLOAD}.{SIGNATURE}"),
            &format!("{HEADER}.bad.{SIGNATURE}"),
            &format!("{HEADER}.{PAYLOAD}.bad"),
            &format!("{HEADER}.!Jmb_nCVJYLD5InaIxsQfS7x87fUsnCYpQK9SrWrKTc.{SIGNATURE}"),
        ] {
            assert_eq!(false, is_valid_jwt(jwt), "{jwt}");
        }
    }

    #[test]
    fn jwt_test_admin_token() {
        assert_eq!(true, is_valid_jwt(Jwt::test_admin_token().as_ref()));
    }

    #[test]
    fn jwt_test_token() {
        assert_eq!(true, is_valid_jwt(Jwt::test_token().as_ref()));
    }

    #[test]
    fn jwt_serde_roundtrip() {
        let jwt_str = format!("{HEADER}.{PAYLOAD}.{SIGNATURE}");
        let json_str = format!("\"{jwt_str}\"");

        let jwt: Jwt = serde_json::from_str(&json_str).unwrap();
        assert_eq!(jwt.as_ref(), jwt_str);
        let json = serde_json::to_string(&jwt).unwrap();
        assert_eq!(json, json_str);

        let err = serde_json::from_str::<Jwt>("\"not-a-jwt\"");
        assert!(err.is_err());
    }
}
