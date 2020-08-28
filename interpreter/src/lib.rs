mod utils;

// https://github.com/pauldix/monkey-rust
mod token;
// mod ast;
// pub mod object;
pub mod lexer;
// pub mod repl;
// pub mod parser;
// pub mod evaluator;
// pub mod code;
// pub mod compiler;
// pub mod vm;


use wasm_bindgen::prelude::*;
// use web_sys::console;

extern crate web_sys;

// A macro to provide `println!(..)`-style syntax for `console.log` logging.
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// This is like the `main` function, except for JavaScript.
#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    log!("Starting up interpreter...");

    Ok(())
}

#[wasm_bindgen]
extern {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Alert from wasm!");
}

#[wasm_bindgen]
pub fn hello() {
    log!("Hello from wasm");
}

#[wasm_bindgen]
pub fn init(msg: &str) -> String {
    utils::set_panic_hook();
    log!("Log from interpreter wasm: {:?}", msg);
    return "Interpreter wasm".to_string()
}