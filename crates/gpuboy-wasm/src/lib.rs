use gpuboy_core::Emulator;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
use gpuboy_render::WgpuRenderer;

thread_local! {
    static EMULATOR: RefCell<Option<Emulator>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static RENDERER: RefCell<Option<WgpuRenderer>> = const { RefCell::new(None) };
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

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn init_renderer(canvas_id: &str) -> js_sys::Promise {
    let canvas_id = canvas_id.to_string();
    wasm_bindgen_futures::future_to_promise(async move {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let canvas: web_sys::HtmlCanvasElement = document
            .get_element_by_id(&canvas_id)
            .ok_or_else(|| JsValue::from_str(&format!("#{} not found", canvas_id)))?
            .dyn_into()
            .map_err(|_| JsValue::from_str("element is not a canvas"))?;
        let renderer = WgpuRenderer::new(canvas)
            .await
            .map_err(|e| JsValue::from_str(&e))?;
        RENDERER.with(|r| *r.borrow_mut() = Some(renderer));
        Ok(JsValue::undefined())
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn render_frame_wgpu(fb: &[u8]) {
    RENDERER.with(|r| {
        if let Some(renderer) = r.borrow().as_ref() {
            renderer.render_frame(fb);
        }
    });
}
