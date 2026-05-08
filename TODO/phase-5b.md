# Phase 5b: Audio Clock

## Overview

Wires the Phase 5a APU into the browser. Replaces the `requestAnimationFrame` timing loop in `index.js` with a `ScriptProcessorNode` audio callback that becomes the master clock. The callback steps the emulator for exactly enough cycles to fill each audio buffer, writes the resulting audio samples to the Web Audio output buffers, and redraws the canvas with the latest framebuffer. This eliminates audio drift and crackling that occur when emulation timing is driven by rAF (which is throttled, jittered, and subject to browser tab visibility).

**Prerequisite:** Phase 5a must be complete and green-lit before starting this phase. The `step_samples` WASM export from Phase 5a is the entry point that drives everything here.

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
www/
  index.js  — add step_samples import; replace rAF loop with ScriptProcessorNode
```

No Rust changes. No new crates. All Rust work was done in Phase 5a.

### Key Decisions

**`ScriptProcessorNode` over `AudioWorklet`.**
`AudioWorklet` requires the audio processor to run in a separate `AudioWorkletGlobalScope`. Sharing the WASM module with that scope requires either: (a) passing a `SharedArrayBuffer` between the main thread and the worklet, which requires `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` HTTP response headers, or (b) a bundler-based setup. Neither is available on a plain GitHub Pages deployment (GitHub Pages serves static files with no custom headers). `ScriptProcessorNode` runs its callback on the main thread where the WASM module already lives, requiring no shared memory or special headers. It is deprecated but universally supported and will not be removed imminently from browsers. A comment in `index.js` acknowledges the deprecation and documents the constraint.

**Audio callback as master clock.**
When rAF drives emulation, the emulator runs ~1 frame worth of cycles per rAF tick. The browser can throttle rAF (hidden tabs, power saving), jitter its timing, or drop frames entirely. The audio subsystem has its own hardware-backed clock and will call `onaudioprocess` on a predictable schedule regardless of tab visibility. By running exactly the right number of CPU cycles per audio callback, audio output and CPU timing are kept in lockstep. Video rendering happens inside the same callback, so it is never faster or slower than the audio clock.

**`bufferSize = 4096`.**
At 44100 Hz, a 4096-sample buffer fires approximately every 92.9 ms (~10.7 times/second). A buffer of 2048 fires ~21.5 times/second but doubles callback overhead and increases the risk of under-runs on slow machines. 4096 provides a comfortable margin with negligible perceptible latency for an emulator. The Game Boy itself runs at ~59.7 fps; the 10.7 Hz video update rate from 4096-sample buffers means roughly 1 video frame rendered per 5-6 Game Boy frames — acceptable because the canvas simply shows the latest completed frame each time the callback fires.

### JS changes (`www/index.js`)

Updated import line (add `step_samples`, keep all existing exports):
```js
import init, { run, load_rom, step_frame, get_framebuffer, init_renderer, render_frame_wgpu, step_samples }
    from "../pkg/gpuboy_wasm.js";
```

Replace the rAF loop with a `ScriptProcessorNode`. The existing `loop()` function and `requestAnimationFrame` calls are removed. Audio setup happens when a ROM is loaded.

Full updated `main()`:

```js
async function main() {
    await init();
    run();

    // Attempt WebGPU init via Rust/wgpu
    let useWebGpu = false;
    try {
        await init_renderer('screen');
        useWebGpu = true;
    } catch (err) {
        console.error('wgpu init failed:', err);
        const errEl = document.getElementById('error');
        if (errEl) {
            errEl.textContent = `WebGPU unavailable: ${err}. Falling back to 2D canvas.`;
            errEl.style.display = 'block';
        }
    }

    document.getElementById('screen').style.display    = useWebGpu ? 'block' : 'none';
    document.getElementById('screen-2d').style.display = useWebGpu ? 'none'  : 'block';
    const ctx2d = document.getElementById('screen-2d').getContext('2d');

    if (useWebGpu) {
        const btn = document.getElementById('renderer-toggle');
        btn.style.display = 'inline';
        btn.addEventListener('click', () => {
            useWebGpu = !useWebGpu;
            document.getElementById('screen').style.display    = useWebGpu ? 'block' : 'none';
            document.getElementById('screen-2d').style.display = useWebGpu ? 'none'  : 'block';
            btn.textContent = useWebGpu ? 'Switch to 2D canvas' : 'Switch to WebGPU';
        });
    }

    // AudioContext is created once, lazily on first ROM load, and reused.
    // ScriptProcessorNode is deprecated but does not require SharedArrayBuffer or
    // COOP/COEP headers, making it compatible with plain GitHub Pages hosting.
    // AudioWorklet would require those headers; see phase-5b spec Key Decisions.
    let audioCtx = null;
    let scriptNode = null;
    let romLoaded = false;

    function startAudio() {
        if (audioCtx) {
            // Already running; nothing to do.
            return;
        }
        audioCtx = new AudioContext({ sampleRate: 44100 });
        const bufferSize = 4096;
        // createScriptProcessor is deprecated but universally supported.
        scriptNode = audioCtx.createScriptProcessor(bufferSize, 0, 2);
        scriptNode.onaudioprocess = (e) => {
            const left  = e.outputBuffer.getChannelData(0);
            const right = e.outputBuffer.getChannelData(1);
            const n = left.length; // == bufferSize
            if (!romLoaded) {
                left.fill(0);
                right.fill(0);
                return;
            }
            const samples = step_samples(n); // Float32Array, length 2*n, interleaved L,R
            for (let i = 0; i < n; i++) {
                left[i]  = samples[i * 2];
                right[i] = samples[i * 2 + 1];
            }
            // Render video from the latest framebuffer produced during this audio tick.
            const fb = get_framebuffer();
            if (useWebGpu) {
                render_frame_wgpu(fb);
            } else {
                render2d(ctx2d, fb);
            }
        };
        scriptNode.connect(audioCtx.destination);
    }

    document.getElementById('rom-picker').addEventListener('change', (e) => {
        const file = e.target.files[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onerror = (ev) => console.error('FileReader error:', ev.target.error);
        reader.onload = (ev) => {
            const data = new Uint8Array(ev.target.result);
            load_rom(data);
            romLoaded = true;
            startAudio();
            // Resume AudioContext if browser suspended it (autoplay policy).
            if (audioCtx.state === 'suspended') {
                audioCtx.resume();
            }
        };
        reader.readAsArrayBuffer(file);
    });
}
```

The `render2d` function is unchanged. The old `loop()` function and `animationId` variable are removed entirely. `step_frame` is no longer called from JS (retained in WASM for existing tests).

## Tasks

- [ ] 1. Update `www/index.js` following §JS changes. Add `step_samples` to the import. Replace the rAF loop with the `ScriptProcessorNode` approach described in §JS changes. Remove the `loop()` function and `animationId` variable. Keep `render2d`, `render_frame_wgpu`, all renderer toggle logic unchanged. Adapt the ROM picker `onload` handler to call `startAudio()` instead of `loop()`. *(req 10, 11, 12)*

- [ ] 2. Verify `www/index.html` requires no changes: the HTML from Phase 3b already has both canvases (`#screen`, `#screen-2d`), the renderer toggle button, the error div, and the `#rom-picker` input. No modifications needed. *(req 10)*

- [ ] 3. Run `cargo clippy` and `cargo fmt -- --check`. Confirm zero warnings and zero errors. *(housekeeping)*

## Manual Testing

1. Build WASM: `wasm-pack build crates/gpuboy-wasm --target web`.
2. Serve: `python -m http.server 8000`. Open `http://localhost:8000/www/` in Chrome or Firefox.
3. Open DevTools. Confirm no errors or warnings on page load.
4. Load `tetris.gb`. Confirm:
   - Audio starts immediately (or after the first user interaction if autoplay is blocked — the `audioCtx.resume()` call in the `change` handler should handle this since it is itself in a user-gesture handler).
   - The Tetris menu music is recognizable: melody on CH1 (pulse), bass line on CH2 (pulse), no buzzing or crackling.
   - The canvas updates continuously (game title screen animates or game runs).
5. Let Tetris run for 30 seconds. Confirm no crackling, stuttering, or audio dropout.
6. Load `pokemon_red.gb` (or `blue`). Confirm the startup jingle plays with multiple instruments. Let it run for 30 seconds with no crackling.
7. Load a CH-isolation test ROM (e.g. `dmg_sound` test 1 from Blargg's suite, or any ROM that exercises one channel at a time). Confirm each of CH1–CH4 produces audible, distinct sound.
8. Open DevTools → Performance (or Network tab throttle to "Slow 3G"). Confirm `requestAnimationFrame` is not called after ROM load (no `rAF` entries in the performance timeline, or confirm the loop function is not present in the call stack during runtime).
9. Test APU off: use a hex editor or test ROM to write 0x00 to 0xFF26. Confirm audio goes silent. Re-enable (write 0x80 to 0xFF26) and confirm channels restart on next trigger.
10. Test sweep: load the Game Boy boot ROM or a ROM that triggers CH1 with sweep (NR10 = 0x1E for a descending sweep). Confirm audible pitch slide at startup.
11. Toggle renderer (if WebGPU is available): click "Switch to 2D canvas" while audio is playing. Confirm audio continues uninterrupted and video renders via the 2D path.
12. Run `cargo clippy` and `cargo fmt -- --check`. Confirm no warnings or errors.

**Green light:** [ ]
