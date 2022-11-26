#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

/// Takes the result of a rsplit and ensure we only get 2 parts
/// Errors if we don't
macro_rules! expect_two {
    ($iter:expr) => {{
        let mut i = $iter;
        match (i.next(), i.next(), i.next()) {
            (Some(first), Some(second), None) => (first, second),
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

    base64::decode_config(header, base64::URL_SAFE).is_ok()
        && base64::decode_config(payload, base64::URL_SAFE).is_ok()
        && base64::decode_config(signature, base64::URL_SAFE).is_ok()
}

#[cfg(test)]
mod test {
    use super::is_valid_jwt;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_jwt() {
        const HEADER: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9";
        const PAYLOAD: &str = "eyJhdWQiOiJhdXRoIiwiZXhwIjoxNjY5Mjk5NjExLCJpYXQiOjE2NjkyOTc4MTEsImlzcyI6ImJlbmNoZXIuZGV2Iiwic3ViIjoiYUBhLmNvIiwib3JnIjpudWxsfQ";
        const SIGNATURE: &str = "jJmb_nCVJYLD5InaIxsQfS7x87fUsnCYpQK9SrWrKTc";

        assert_eq!(
            true,
            is_valid_jwt(&format!("{HEADER}.{PAYLOAD}.{SIGNATURE}"))
        );
        assert_eq!(true, is_valid_jwt(&format!(".{PAYLOAD}.{SIGNATURE}")));
        assert_eq!(true, is_valid_jwt(&format!("{HEADER}..{SIGNATURE}")));
        assert_eq!(true, is_valid_jwt(&format!("{HEADER}.{PAYLOAD}.")));
        assert_eq!(true, is_valid_jwt(&format!("{HEADER}..")));
        assert_eq!(true, is_valid_jwt(&format!(".{PAYLOAD}.")));
        assert_eq!(true, is_valid_jwt(&format!("..{SIGNATURE}")));

        assert_eq!(
            false,
            is_valid_jwt(&format!(" {HEADER}.{PAYLOAD}.{SIGNATURE}"))
        );
        assert_eq!(
            false,
            is_valid_jwt(&format!("{HEADER}.{PAYLOAD}.{SIGNATURE} "))
        );
        assert_eq!(false, is_valid_jwt(&format!("{HEADER}.{PAYLOAD}")));
        assert_eq!(false, is_valid_jwt(&format!("{HEADER}.")));
        assert_eq!(false, is_valid_jwt(&format!("{PAYLOAD}.{SIGNATURE}")));
        assert_eq!(false, is_valid_jwt(&format!(".{SIGNATURE}")));
        assert_eq!(false, is_valid_jwt(&format!("bad.{PAYLOAD}.{SIGNATURE}")));
        assert_eq!(false, is_valid_jwt(&format!("{HEADER}.bad.{SIGNATURE}")));
        assert_eq!(false, is_valid_jwt(&format!("{HEADER}.{PAYLOAD}.bad")));
        assert_eq!(
            false,
            is_valid_jwt(&format!(
                "{HEADER}.!Jmb_nCVJYLD5InaIxsQfS7x87fUsnCYpQK9SrWrKTc.{SIGNATURE}"
            ))
        );
    }
}
