#![cfg(feature = "wasm")]

use std::str::FromStr as _;

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(start)]
pub fn startup() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn is_valid_uuid(uuid: &str) -> bool {
    uuid::Uuid::from_str(uuid).is_ok()
}

// In order to avoid dynamic imports in Bencher Console,
// fill-in Bencher Plus functions when the `plus` feature is not enabled.
pub mod not_plus {
    #![cfg(not(feature = "plus"))]

    use wasm_bindgen::prelude::wasm_bindgen;

    #[wasm_bindgen]
    pub fn is_valid_plan_level(_plan_level: &str) -> bool {
        false
    }
}
