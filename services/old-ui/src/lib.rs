pub use reports::{InventoryData, Reports};
use wasm_bindgen::prelude::*;

mod utils;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(start)]
pub fn startup() {
    utils::set_panic_hook();
    console_log::init_with_level(log::Level::Debug).expect("Failed to initialize console log.");
    log::debug!("Initializing Bencher...")
}
