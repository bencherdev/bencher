#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

mod email;
mod jwt;
mod user_name;

pub use email::is_valid_email;
pub use jwt::is_valid_jwt;
pub use user_name::{is_valid_user_name, UserName};

const REGEX_ERROR: &str = "Failed to compile regex.";

#[cfg(feature = "wasm")]
#[wasm_bindgen(start)]
pub fn startup() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}
