mod utils;

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg_attr(feature = "wasm", wasm_bindgen(start))]
pub fn startup() {
    utils::set_panic_hook();
    console_log::init_with_level(log::Level::Debug).expect("Error init console log");
    log::debug!("Bencher Validation");
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn validate_email(email: &str) -> bool {
    true
}
