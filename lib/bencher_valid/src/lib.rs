#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

mod email;
mod error;
mod jwt;
mod user_name;

pub use email::{is_valid_email, Email};
pub use error::ValidError;
use error::REGEX_ERROR;
pub use jwt::is_valid_jwt;
pub use user_name::{is_valid_user_name, UserName};

#[cfg(feature = "wasm")]
#[wasm_bindgen(start)]
pub fn startup() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}
