#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

mod email;
mod error;
mod jwt;
mod resource_id;
mod slug;
mod user_name;

pub use crate::slug::Slug;
pub use email::Email;
pub use error::ValidError;
use error::REGEX_ERROR;
pub use jwt::Jwt;
pub use resource_id::ResourceId;
pub use user_name::UserName;

#[cfg(feature = "wasm")]
#[wasm_bindgen(start)]
pub fn startup() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}
