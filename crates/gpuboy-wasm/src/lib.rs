use gpuboy_core::Emulator;
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
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
    static AUDIO_CTX: RefCell<Option<web_sys::AudioContext>> = const { RefCell::new(None) };
    static SCRIPT_NODE: RefCell<Option<web_sys::ScriptProcessorNode>> = const { RefCell::new(None) };
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

// web-sys's AudioBuffer::get_channel_data returns Vec<f32> (a copy), so writing to it has no
// effect on the output buffer. This helper calls getChannelData().set() directly in JS so that
// writes reach the actual Float32Array views inside the AudioBuffer.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = "
export function audio_write_channels(output_buffer, left_data, right_data) {
    output_buffer.getChannelData(0).set(left_data);
    output_buffer.getChannelData(1).set(right_data);
}
")]
extern "C" {
    fn audio_write_channels(
        output: &web_sys::AudioBuffer,
        left: &js_sys::Float32Array,
        right: &js_sys::Float32Array,
    );
}

/// Sets up the `ScriptProcessorNode` audio clock. Idempotent: safe to call on every ROM load.
///
/// `on_frame` is a JS function called once per audio buffer with a `Uint8Array` containing the
/// latest framebuffer (RGBA, 160×144). The caller is responsible for rendering it to a canvas.
///
/// `ScriptProcessorNode` is deprecated but runs on the main thread where the WASM module lives,
/// requiring no `SharedArrayBuffer` or COOP/COEP headers — both unavailable on GitHub Pages.
/// `AudioWorklet` would need those headers. See phase-5b spec § Key Decisions.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start_audio(on_frame: js_sys::Function) -> Result<(), JsValue> {
    let already_started = AUDIO_CTX.with(|a| a.borrow().is_some());
    if already_started {
        return Ok(());
    }

    let mut opts = web_sys::AudioContextOptions::new();
    opts.set_sample_rate(44100.0);
    let ctx = web_sys::AudioContext::new_with_context_options(&opts)?;

    const BUFFER_SIZE: u32 = 4096;
    let script_node = ctx
        .create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(
            BUFFER_SIZE, 0, 2,
        )?;

    let n = BUFFER_SIZE as usize;
    let closure = Closure::wrap(Box::new(move |event: web_sys::AudioProcessingEvent| {
        let output = match event.output_buffer() {
            Ok(buf) => buf,
            Err(_) => return,
        };
        let maybe_samples =
            EMULATOR.with(|e| e.borrow_mut().as_mut().map(|emu| emu.step_samples(n)));
        match maybe_samples {
            Some(samples) => {
                let left_vec: Vec<f32> = (0..n).map(|i| samples[i * 2]).collect();
                let right_vec: Vec<f32> = (0..n).map(|i| samples[i * 2 + 1]).collect();
                audio_write_channels(
                    &output,
                    &js_sys::Float32Array::from(left_vec.as_slice()),
                    &js_sys::Float32Array::from(right_vec.as_slice()),
                );
                let fb = EMULATOR.with(|e| {
                    e.borrow()
                        .as_ref()
                        .map(|emu| emu.get_framebuffer().to_vec())
                });
                if let Some(fb) = fb {
                    let fb_js: JsValue = js_sys::Uint8Array::from(fb.as_slice()).into();
                    let _ = on_frame.call1(&JsValue::NULL, &fb_js);
                }
            }
            None => {
                let silence = js_sys::Float32Array::new_with_length(n as u32);
                audio_write_channels(&output, &silence, &silence);
            }
        }
    }) as Box<dyn FnMut(web_sys::AudioProcessingEvent)>);

    script_node.set_onaudioprocess(Some(closure.as_ref().unchecked_ref()));
    closure.forget();

    script_node.connect_with_audio_node(&ctx.destination())?;
    let _: Result<_, _> = ctx.resume();

    AUDIO_CTX.with(|a| *a.borrow_mut() = Some(ctx));
    SCRIPT_NODE.with(|s| *s.borrow_mut() = Some(script_node));
    Ok(())
}
