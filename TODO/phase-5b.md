# Phase 5b: Audio Clock

## Overview

Wires the Phase 5a APU into the browser. Replaces the `requestAnimationFrame` timing loop in `index.js` with a `ScriptProcessorNode` audio callback that becomes the master clock. The callback steps the emulator for exactly enough cycles to fill each audio buffer, writes the resulting audio samples to the Web Audio output buffers, and redraws the canvas with the latest framebuffer. This eliminates audio drift and crackling that occur when emulation timing is driven by rAF (which is throttled, jittered, and subject to browser tab visibility).

**Prerequisite:** Phase 5a must be complete and green-lit before starting this phase. `Emulator::step_samples` in `gpuboy-core` is the entry point that drives everything here.

## Requirements

10. WHEN a ROM is loaded and audio is started, THEN the `requestAnimationFrame` loop is cancelled and replaced by a `ScriptProcessorNode` `onaudioprocess` callback that drives all emulation timing.

11. WHEN the `onaudioprocess` callback fires with a buffer of N stereo samples, THEN the emulator is stepped for exactly N sample-pairs worth of CPU cycles, the resulting audio is written to both output channels, and the canvas is redrawn with the latest framebuffer.

12. WHEN no ROM is loaded, THEN the `onaudioprocess` callback fills the output buffers with silence (0.0) and does not call into WASM emulator state.

## Acceptance Criteria

- [ ] Tetris menu music plays recognizably (all channels audible, melody on CH1, bass on CH2).
- [ ] Pokemon Red/Blue intro jingle plays without crackling for at least 30 seconds.
- [ ] All four channels produce audible, distinct output when loaded with a test ROM that exercises each channel independently (e.g. `dmg_sound` test ROM suite or a known audio test ROM).
- [ ] No audio crackling in the first 30 seconds of any test ROM.
- [ ] The canvas updates continuously while audio is playing (video is not frozen).
- [ ] `requestAnimationFrame` is not called after a ROM is loaded and the audio node starts.
- [ ] With NR52 bit 7 = 0 (sound off), the output is silence and no channel state is updated.
- [ ] CH1 sweep: a ROM that uses frequency sweep (e.g. the descending sweep at Game Boy boot) produces an audible pitch slide.
- [ ] Length counters: a channel with length enable set and a short length value goes silent at the correct time (audible as a note cut-off).
- [ ] Volume envelopes: CH1/CH2/CH4 envelopes produce fade-in or fade-out effects as programmed.
- [ ] No regressions: video still renders correctly via the wgpu path (or 2D fallback); ROM loading still works.

## Design

### Architecture

```
crates/
  gpuboy-wasm/
    Cargo.toml  — add web-sys audio features
    src/lib.rs  — add start_audio export; add AUDIO_CTX + SCRIPT_NODE thread-locals
www/
  index.js      — replace rAF loop with start_audio call; pass render callback
```

No changes to `gpuboy-core` or `gpuboy-render`. No new crates.

### Key Decisions

**`ScriptProcessorNode` over `AudioWorklet`.**
`AudioWorklet` requires the audio processor to run in a separate `AudioWorkletGlobalScope`. Sharing the WASM module with that scope requires either: (a) passing a `SharedArrayBuffer` between the main thread and the worklet, which requires `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` HTTP response headers, or (b) a bundler-based setup. Neither is available on a plain GitHub Pages deployment (GitHub Pages serves static files with no custom headers). `ScriptProcessorNode` runs its callback on the main thread where the WASM module already lives, requiring no shared memory or special headers. It is deprecated but universally supported and will not be removed imminently from browsers. A comment in `lib.rs` acknowledges the deprecation and documents the constraint.

**Audio callback implemented in Rust via `web-sys`.**
The `AudioContext`, `ScriptProcessorNode`, and `onaudioprocess` closure all live in `gpuboy-wasm/src/lib.rs`. The closure captures a `js_sys::Function` callback that JS passes in for video rendering; everything else (emulator stepping, sample de-interleaving, framebuffer retrieval) happens in Rust. `index.js` is kept as thin as possible: it passes a render function and nothing more. The `Closure` is leaked with `forget()` so it lives for the lifetime of the page.

**Audio callback as master clock.**
When rAF drives emulation, the emulator runs ~1 frame worth of cycles per rAF tick. The browser can throttle rAF (hidden tabs, power saving), jitter its timing, or drop frames entirely. The audio subsystem has its own hardware-backed clock and will call `onaudioprocess` on a predictable schedule regardless of tab visibility. By running exactly the right number of CPU cycles per audio callback, audio output and CPU timing are kept in lockstep. Video rendering happens inside the same callback, so it is never faster or slower than the audio clock.

**`bufferSize = 4096`.**
At 44100 Hz, a 4096-sample buffer fires approximately every 92.9 ms (~10.7 times/second). A buffer of 2048 fires ~21.5 times/second but doubles callback overhead and increases the risk of under-runs on slow machines. 4096 provides a comfortable margin with negligible perceptible latency for an emulator. The Game Boy itself runs at ~59.7 fps; the 10.7 Hz video update rate from 4096-sample buffers means roughly 1 video frame rendered per 5-6 Game Boy frames — acceptable because the canvas simply shows the latest completed frame each time the callback fires.

### Rust changes (`crates/gpuboy-wasm/`)

**`Cargo.toml`** — extend the `web-sys` features list:
```toml
[dependencies.web-sys]
version = "0.3"
features = [
    "console",
    "Window",
    "Document",
    "HtmlCanvasElement",
    "AudioContext",
    "AudioContextOptions",
    "AudioNode",
    "AudioBuffer",
    "AudioProcessingEvent",
    "ScriptProcessorNode",
    "AudioDestinationNode",
]
```

**`src/lib.rs`** — add two new thread-locals and the `start_audio` export:

```rust
#[cfg(target_arch = "wasm32")]
thread_local! {
    static AUDIO_CTX: RefCell<Option<web_sys::AudioContext>> = const { RefCell::new(None) };
    static SCRIPT_NODE: RefCell<Option<web_sys::ScriptProcessorNode>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start_audio(on_frame: js_sys::Function) -> Result<(), JsValue> {
    // Idempotent: only create AudioContext once.
    let already_started = AUDIO_CTX.with(|a| a.borrow().is_some());
    if already_started {
        return Ok(());
    }

    let mut opts = web_sys::AudioContextOptions::new();
    opts.sample_rate(44100.0);
    let ctx = web_sys::AudioContext::new_with_context_options(&opts)?;

    const BUFFER_SIZE: u32 = 4096;
    // ScriptProcessorNode is deprecated but runs on the main thread where WASM lives,
    // requiring no SharedArrayBuffer or COOP/COEP headers (incompatible with GitHub Pages).
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
        let maybe_samples = EMULATOR.with(|e| {
            e.borrow_mut().as_mut().map(|emu| emu.step_samples(n))
        });
        match maybe_samples {
            Some(samples) => {
                if let (Ok(left_ch), Ok(right_ch)) =
                    (output.get_channel_data(0), output.get_channel_data(1))
                {
                    let left_vec: Vec<f32>  = (0..n).map(|i| samples[i * 2]).collect();
                    let right_vec: Vec<f32> = (0..n).map(|i| samples[i * 2 + 1]).collect();
                    left_ch.copy_from(&left_vec);
                    right_ch.copy_from(&right_vec);
                }
                let fb = EMULATOR.with(|e| {
                    e.borrow().as_ref().map(|emu| emu.get_framebuffer().to_vec())
                });
                if let Some(fb) = fb {
                    let fb_js: JsValue = js_sys::Uint8Array::from(fb.as_slice()).into();
                    let _ = on_frame.call1(&JsValue::NULL, &fb_js);
                }
            }
            None => {
                if let (Ok(left_ch), Ok(right_ch)) =
                    (output.get_channel_data(0), output.get_channel_data(1))
                {
                    left_ch.copy_from(&vec![0.0f32; n]);
                    right_ch.copy_from(&vec![0.0f32; n]);
                }
            }
        }
    }) as Box<dyn FnMut(web_sys::AudioProcessingEvent)>);

    script_node.set_onaudioprocess(Some(closure.as_ref().unchecked_ref()));
    closure.forget();

    script_node.connect_with_audio_node(&ctx.destination())?;
    // Fire-and-forget resume in case autoplay policy suspended the context.
    let _: Result<_, _> = ctx.resume();

    AUDIO_CTX.with(|a| *a.borrow_mut() = Some(ctx));
    SCRIPT_NODE.with(|s| *s.borrow_mut() = Some(script_node));
    Ok(())
}
```

### JS changes (`www/index.js`)

Updated import (add `start_audio`, drop `step_frame` — no longer called from JS):
```js
import init, { run, load_rom, get_framebuffer, init_renderer, render_frame_wgpu, start_audio }
    from "../pkg/gpuboy_wasm.js";
```

Remove `loop()` and `animationId`. In the ROM picker `onload` handler, call `start_audio` with a render callback that receives the framebuffer `Uint8Array` from Rust:

```js
reader.onload = (ev) => {
    const data = new Uint8Array(ev.target.result);
    load_rom(data);
    start_audio((fb) => {
        if (useWebGpu) {
            render_frame_wgpu(fb);
        } else {
            render2d(ctx2d, fb);
        }
    });
};
```

`render2d` and all renderer-toggle logic are unchanged.

## Tasks

- [x] 1. Update `www/index.js` following §JS changes. Add `step_samples` to the import. Replace the rAF loop with the `ScriptProcessorNode` approach described in §JS changes. Remove the `loop()` function and `animationId` variable. Keep `render2d`, `render_frame_wgpu`, all renderer toggle logic unchanged. Adapt the ROM picker `onload` handler to call `startAudio()` instead of `loop()`. *(req 10, 11, 12)*

- [x] 2. Verify `www/index.html` requires no changes: the HTML from Phase 3b already has both canvases (`#screen`, `#screen-2d`), the renderer toggle button, the error div, and the `#rom-picker` input. No modifications needed. *(req 10)*

- [x] 3. Extend `web-sys` features in `crates/gpuboy-wasm/Cargo.toml` to include the audio types: `AudioContext`, `AudioContextOptions`, `AudioNode`, `AudioBuffer`, `AudioProcessingEvent`, `ScriptProcessorNode`, `AudioDestinationNode`. *(housekeeping)*

- [x] 4. Add `use wasm_bindgen::closure::Closure;` import and `AUDIO_CTX` / `SCRIPT_NODE` thread-locals to `crates/gpuboy-wasm/src/lib.rs`. *(req 10)*

- [x] 5. Implement `start_audio(on_frame: js_sys::Function) -> Result<(), JsValue>` in `crates/gpuboy-wasm/src/lib.rs` following §Rust changes: create `AudioContext` (44100 Hz), create `ScriptProcessorNode` (bufferSize=4096, 0 inputs, 2 outputs), attach `onaudioprocess` Rust closure that steps the emulator, de-interleaves samples into L/R channels, gets the framebuffer, calls `on_frame(fb_uint8array)`. Silence path when no ROM loaded. `closure.forget()`. Connect node to destination. Fire-and-forget resume. Store ctx and node in thread-locals. *(req 10, 11, 12)*

- [x] 6. Update `www/index.js` following §JS changes: add `start_audio` to import (drop `step_frame`), remove `loop()` and `animationId`, update ROM picker `onload` to call `start_audio((fb) => { ... })`. *(req 10, 11, 12)*

- [ ] 7. Run `cargo clippy` and `cargo fmt -- --check`. Confirm zero warnings and zero errors. *(housekeeping)*

## Manual Testing

1. Build WASM: `wasm-pack build crates/gpuboy-wasm --target web`.
2. Serve: `python -m http.server 8000`. Open `http://localhost:8000/www/` in Chrome or Firefox.
3. Open DevTools. Confirm no errors or warnings on page load.
4. Load `tetris.gb`. Confirm:
   - Audio starts immediately (or after the first user interaction if autoplay is blocked — `start_audio` calls `resume()` inside a user-gesture handler).
   - The Tetris menu music is recognizable: melody on CH1 (pulse), bass line on CH2 (pulse), no buzzing or crackling.
   - The canvas updates continuously (game title screen animates or game runs).
5. Let Tetris run for 30 seconds. Confirm no crackling, stuttering, or audio dropout.
6. Load `pokemon_red.gb` (or `blue`). Confirm the startup jingle plays with multiple instruments. Let it run for 30 seconds with no crackling.
7. Load a CH-isolation test ROM (e.g. `dmg_sound` test 1 from Blargg's suite, or any ROM that exercises one channel at a time). Confirm each of CH1–CH4 produces audible, distinct sound.
8. Open DevTools → Performance. Confirm `requestAnimationFrame` is not called after ROM load.
9. Test APU off: use a hex editor or test ROM to write 0x00 to 0xFF26. Confirm audio goes silent. Re-enable (write 0x80 to 0xFF26) and confirm channels restart on next trigger.
10. Test sweep: load the Game Boy boot ROM or a ROM that triggers CH1 with sweep (NR10 = 0x1E for a descending sweep). Confirm audible pitch slide at startup.
11. Toggle renderer (if WebGPU is available): click "Switch to 2D canvas" while audio is playing. Confirm audio continues uninterrupted and video renders via the 2D path.
12. Run `cargo clippy` and `cargo fmt -- --check`. Confirm no warnings or errors.

**Green light:** [x]
