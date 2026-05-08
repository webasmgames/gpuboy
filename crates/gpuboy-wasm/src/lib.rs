use gpuboy_core::Emulator;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {
    static EMULATOR: RefCell<Option<Emulator>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub fn run() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"gpuboy ready".into());
}

#[wasm_bindgen]
pub fn load_rom(data: Vec<u8>) {
    // Title is at 0x0134–0x0143 (16 bytes, zero-padded ASCII)
    let title = data
        .get(0x134..0x144)
        .map(|b| {
            String::from_utf8_lossy(b)
                .trim_end_matches('\0')
                .to_string()
        })
        .unwrap_or_default();
    match Emulator::new(data) {
        Ok(emu) => {
            EMULATOR.with(|e| *e.borrow_mut() = Some(emu));
            web_sys::console::log_1(&title.into());
        }
        Err(e) => web_sys::console::log_1(&e.into()),
    }
}

#[wasm_bindgen]
pub fn step_frame() {
    EMULATOR.with(|e| {
        if let Some(emu) = e.borrow_mut().as_mut() {
            emu.step_frame();
        }
    });
}

#[wasm_bindgen]
pub fn get_framebuffer() -> Vec<u8> {
    EMULATOR.with(|e| {
        if let Some(emu) = e.borrow().as_ref() {
            emu.get_framebuffer().to_vec()
        } else {
            Vec::new()
        }
    })
}

#[wasm_bindgen]
pub fn take_serial_output() -> String {
    EMULATOR.with(|e| {
        if let Some(emu) = e.borrow_mut().as_mut() {
            let bytes = emu.take_serial_output();
            let s = String::from_utf8_lossy(&bytes).into_owned();
            web_sys::console::log_1(&s.clone().into());
            s
        } else {
            String::new()
        }
    })
}
