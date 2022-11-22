use email_address_parser::EmailAddress;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
mod utils;

#[cfg(feature = "wasm")]
#[wasm_bindgen(start)]
pub fn startup() {
    utils::set_panic_hook();

    #[cfg(debug_assertions)]
    let log_level = log::Level::Debug;
    #[cfg(not(debug_assertions))]
    let log_level = log::Level::Info;
    console_log::init_with_level(log_level).expect("Error init console log");
    log::debug!("Bencher Validation");
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_email(email: &str) -> bool {
    EmailAddress::parse(email, None).is_some()
}
