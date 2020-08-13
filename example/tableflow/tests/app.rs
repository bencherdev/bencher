use wasm_bindgen_test::{wasm_bindgen_test_configure, wasm_bindgen_test};
use futures::prelude::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

wasm_bindgen_test_configure!(run_in_browser);


// This runs a unit test in native Rust, so it can only use Rust APIs.
#[test]
fn rust_test() {
    assert_eq!(1, 1);
}


// This runs a unit test in the browser, so it can use browser APIs.
#[wasm_bindgen_test]
fn web_test() {
    assert_eq!(1, 1);
}


// This runs a unit test in the browser, and in addition it supports asynchronous Future APIs.
#[wasm_bindgen_test(async)]
fn async_test() -> impl Future<Item = (), Error = JsValue> {
    // Creates a JavaScript Promise which will asynchronously resolve with the value 42.
    let promise = js_sys::Promise::resolve(&JsValue::from(42));

    // Converts that Promise into a Future.
    // The unit test will wait for the Future to resolve.
    JsFuture::from(promise)
        .map(|x| {
            assert_eq!(x, 42);
        })
}
