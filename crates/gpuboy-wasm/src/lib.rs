use gpuboy_core::cartridge::Cartridge;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"gpuboy ready".into());
}

#[wasm_bindgen]
pub fn load_rom(data: Vec<u8>) {
    match Cartridge::load(data) {
        Ok(cart) => web_sys::console::log_1(&cart.header.title.into()),
        Err(e) => web_sys::console::log_1(&e.into()),
    }
}
