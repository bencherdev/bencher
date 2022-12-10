#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

mod branch_name;
mod email;
mod error;
mod git_hash;
mod jwt;
mod non_empty;
mod resource_id;
mod slug;
mod url;
mod user_name;

pub use crate::git_hash::GitHash;
pub use crate::slug::Slug;
pub use crate::url::Url;
pub use branch_name::BranchName;
pub use email::Email;
pub use error::ValidError;
use error::REGEX_ERROR;
pub use jwt::Jwt;
pub use non_empty::NonEmpty;
pub use resource_id::ResourceId;
pub use user_name::UserName;

const MAX_LEN: usize = 50;

#[cfg(feature = "wasm")]
#[wasm_bindgen(start)]
pub fn startup() {
    console_error_panic_hook::set_once();
}

fn is_valid_len(input: &str) -> bool {
    !input.is_empty() && input.len() <= MAX_LEN && input == input.trim()
}

#[cfg(test)]
mod test {

    use super::is_valid_len;
    use pretty_assertions::assert_eq;

    const LEN_0_STR: &str = "";
    const LEN_1_STR: &str = "0";
    const LEN_2_STR: &str = "01";
    const LEN_3_STR: &str = "012";
    const LEN_50_STR: &str = "01234567890123456789012345678901234567890123456789";
    const LEN_51_STR: &str = "012345678901234567890123456789012345678901234567890";

    const PRE_SPACE_STR: &str = " 0123";
    const POST_SPACE_STR: &str = "0123 ";
    const BOTH_SPACE_STR: &str = " 0123 ";
    const INNER_SPACE_STR: &str = "01 23";

    #[test]
    fn test_is_valid_len() {
        assert_eq!(false, is_valid_len(LEN_0_STR));
        assert_eq!(true, is_valid_len(LEN_1_STR));
        assert_eq!(true, is_valid_len(LEN_2_STR));
        assert_eq!(true, is_valid_len(LEN_3_STR));
        assert_eq!(true, is_valid_len(LEN_50_STR));
        assert_eq!(false, is_valid_len(LEN_51_STR));

        assert_eq!(false, is_valid_len(PRE_SPACE_STR));
        assert_eq!(false, is_valid_len(POST_SPACE_STR));
        assert_eq!(false, is_valid_len(BOTH_SPACE_STR));
        assert_eq!(true, is_valid_len(INNER_SPACE_STR));
    }
}
