use email_address_parser::EmailAddress;
use once_cell::sync::Lazy;
use regex::Regex;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

const REGEX_ERROR: &str = "Failed to compile regex.";

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

    let trim_name = name.trim();
    if trim_name.len() < 4 || trim_name.len() > 50 {
        return false;
    };

    return NAME_REGEX.is_match(trim_name);
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_email(email: &str) -> bool {
    EmailAddress::parse(email, None).is_some()
}

#[cfg(test)]
mod test {
    use super::{is_valid_email, is_valid_user_name};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_user_name() {
        assert_eq!(true, is_valid_user_name("muriel"));
        assert_eq!(true, is_valid_user_name("Muriel"));
        assert_eq!(true, is_valid_user_name("Muriel Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel Linda Bagge"));
        assert_eq!(true, is_valid_user_name("Bagge, Muriel"));
        assert_eq!(true, is_valid_user_name("Mrs. Muriel Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel Linda-Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel De'Bagge"));
        assert_eq!(true, is_valid_user_name(" Muriel Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel  Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel Bagge "));
        assert_eq!(true, is_valid_user_name(" Muriel Bagge "));
        assert_eq!(true, is_valid_user_name("Mrs. Muriel Linda-De'Bagge"));

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

        assert_eq!(false, is_valid_email("example.com"));
        assert_eq!(false, is_valid_email("abc example.com"));
        assert_eq!(false, is_valid_email("abc.example.com"));
        assert_eq!(false, is_valid_email("abc!example.com"));
    }
}
