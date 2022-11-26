use email_address::EmailAddress;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_email(email: &str) -> bool {
    EmailAddress::is_valid(email)
}

#[cfg(test)]
mod test {
    use super::is_valid_email;
    use pretty_assertions::assert_eq;

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
}
