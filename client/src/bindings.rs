use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/chart.js")]
extern "C" {
    #[wasm_bindgen(js_name = "show_chart")]
    pub fn show_chart(chart: JsValue);
}
