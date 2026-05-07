use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"gpuboy ready".into());
}
