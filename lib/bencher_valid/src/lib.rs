use email_address::EmailAddress;
use once_cell::sync::Lazy;
use regex::Regex;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

mod user_name;

const REGEX_ERROR: &str = "Failed to compile regex.";

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

#[cfg(feature = "wasm")]
#[wasm_bindgen(start)]
pub fn startup() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    #[cfg(debug_assertions)]
    let log_level = log::Level::Debug;
    #[cfg(not(debug_assertions))]
    let log_level = log::Level::Info;

    console_log::init_with_level(log_level).expect("Error init console log");
    log::debug!("Bencher Validation");
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_user_name(name: &str) -> bool {
    static NAME_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[[[:alnum:]] ,\.\-']{4,50}$").expect(REGEX_ERROR));

    if name != name.trim() {
        return false;
    }

    if name.len() < 4 || name.len() > 50 {
        return false;
    };

    NAME_REGEX.is_match(name)
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_email(email: &str) -> bool {
    EmailAddress::is_valid(email)
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
    use super::{is_valid_email, is_valid_jwt, is_valid_user_name};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_user_name() {
        assert_eq!(true, is_valid_user_name("muriel"));
        assert_eq!(true, is_valid_user_name("Muriel"));
        assert_eq!(true, is_valid_user_name("Muriel Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel    Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel Linda Bagge"));
        assert_eq!(true, is_valid_user_name("Bagge, Muriel"));
        assert_eq!(true, is_valid_user_name("Mrs. Muriel Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel Linda-Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel De'Bagge"));
        assert_eq!(true, is_valid_user_name("Mrs. Muriel Linda-De'Bagge"));

        assert_eq!(false, is_valid_user_name(" Muriel Bagge"));
        assert_eq!(false, is_valid_user_name("Muriel Bagge "));
        assert_eq!(false, is_valid_user_name(" Muriel Bagge "));
        assert_eq!(false, is_valid_user_name("Muriel!"));
        assert_eq!(false, is_valid_user_name("Muriel! Bagge"));
        assert_eq!(true, is_valid_user_name("Dumb"));
        assert_eq!(false, is_valid_user_name("Dog"));
        assert_eq!(
            true,
            is_valid_user_name("01234567890123456789012345678901234567890123456789")
        );
        assert_eq!(
            false,
            is_valid_user_name("012345678901234567890123456789012345678901234567890")
        );
        assert_eq!(false, is_valid_user_name(""));
    }

    #[test]
    fn test_email() {
        assert_eq!(true, is_valid_email("abc.xyz@example.com"));
        assert_eq!(true, is_valid_email("abc@example.com"));
        assert_eq!(true, is_valid_email("a@example.com"));
        assert_eq!(true, is_valid_email("abc.xyz@example.co"));
        assert_eq!(true, is_valid_email("abc@example.co"));
        assert_eq!(true, is_valid_email("a@example.co"));
        assert_eq!(true, is_valid_email("abc.xyz@example"));
        assert_eq!(true, is_valid_email("abc@example"));
        assert_eq!(true, is_valid_email("a@example"));

        assert_eq!(false, is_valid_email(" abc@example.com"));
        assert_eq!(false, is_valid_email("abc @example.com"));
        assert_eq!(false, is_valid_email("abc@example.com "));
        assert_eq!(false, is_valid_email("example.com"));
        assert_eq!(false, is_valid_email("abc.example.com"));
        assert_eq!(false, is_valid_email("abc!example.com"));
    }

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
