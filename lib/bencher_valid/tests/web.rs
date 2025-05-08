#![expect(unused_crate_dependencies)]
#![cfg(target_arch = "wasm32")]
//! Test suite for the Web and headless browsers.

extern crate wasm_bindgen_test;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn pass() {
    assert_eq!(1 + 1, 2);
}
