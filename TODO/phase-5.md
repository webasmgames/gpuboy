# Phase 5: APU + Web Audio

## Overview

Implements the Game Boy Audio Processing Unit (APU) across all four channels — CH1 (pulse + sweep), CH2 (pulse), CH3 (wave), CH4 (noise) — and replaces the `requestAnimationFrame` timing loop with a `ScriptProcessorNode` audio callback. The audio callback becomes the master clock: it steps the emulator for exactly enough cycles to fill each audio buffer, then renders the resulting framebuffer to the canvas. This eliminates audio drift and crackling that occur when emulation timing is driven by rAF (which is throttled, jittered, and subject to browser tab visibility). Video continues to update on every audio callback, which fires at a rate determined by the audio hardware (~11 times/second at 44100 Hz / 4096 samples).

## Requirements

1. WHEN the APU registers NR10–NR14, NR21–NR24, NR30–NR34, NR41–NR44, NR50, NR51, NR52, and wave RAM (0xFF30–0xFF3F) are written by the CPU, THEN the APU stores and applies those values to channel state.

2. WHEN NR52 bit 7 (sound enable) is clear, THEN all channel DACs are disabled and `step_apu` produces silence (0.0 samples) without updating channel state.

3. WHEN the emulator is running and NR52 bit 7 is set, THEN the APU generates one stereo f32 sample pair (left, right) every `4194304 / sample_rate` CPU T-cycles, using partial-cycle accumulation to handle the non-integer ratio at 44100 Hz.

4. WHEN CH1 is enabled (NR52 bit 0 set) and its length counter reaches zero with length enable set (NR14 bit 6), THEN CH1 is disabled and produces 0.0 output until retriggered.

5. WHEN CH1 is retriggered (NR14 bit 7 written), THEN its length counter, envelope, frequency timer, and sweep unit are reloaded from their respective NR1x registers.

6. WHEN CH1's sweep unit is active (period > 0 or shift > 0) and its sweep timer expires, THEN the frequency is recalculated; if the new 11-bit frequency overflows 0x7FF, CH1 is disabled.

7. WHEN CH2, CH3, or CH4 are triggered via their respective NRx4 bit 7 writes, THEN their length counters, envelopes (CH2/CH4), and timers are reloaded from the corresponding NRx registers.

8. WHEN the frame sequencer ticks (every 8192 CPU T-cycles), THEN: length counters tick at steps 0, 2, 4, 6 (256 Hz); the CH1 sweep unit ticks at steps 2, 6 (128 Hz); volume envelopes tick at step 7 (64 Hz).

9. WHEN NR50 (master volume), NR51 (panning), or individual channel enables in NR52 change, THEN the mixed output levels adjust accordingly on the next generated sample.

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
Cargo workspace (unchanged):
  crates/
    gpuboy-core/    — add apu.rs module; Bus routes APU register reads/writes;
                      Emulator::step clocks the APU; new step_samples() on Emulator
    gpuboy-wasm/    — new export: step_samples(n: usize) -> Vec<f32>
  www/
    index.js        — replace rAF loop with ScriptProcessorNode; updated import
```

`gpuboy-core` gains a new `apu.rs` module containing `Apu`, `Ch1`, `Ch2`, `Ch3`, `Ch4`. The `Bus` holds an `Apu` instance, routes reads/writes for 0xFF10–0xFF3F, and exposes `step_apu(t_cycles, samples_out)`. `Emulator` gets a new method `step_samples(n: usize) -> Vec<f32>` that runs the emulator for exactly `n` stereo sample-pairs and returns interleaved `[L0, R0, L1, R1, ...]`.

No new crate is needed. The APU is pure logic with no WASM or web-sys dependencies.

### Data Structures

#### `Apu` (in `crates/gpuboy-core/src/apu.rs`)

```rust
pub struct Apu {
    pub enabled: bool,          // NR52 bit 7

    pub ch1: Ch1,
    pub ch2: Ch2,
    pub ch3: Ch3,
    pub ch4: Ch4,

    // NR50: left/right master volume (bits 6-4 = left vol 0-7, bits 2-0 = right vol 0-7)
    pub nr50: u8,
    // NR51: channel panning (bit 7=CH4-L, 6=CH3-L, 5=CH2-L, 4=CH1-L,
    //                         3=CH4-R, 2=CH3-R, 1=CH2-R, 0=CH1-R)
    pub nr51: u8,

    // Frame sequencer: counts CPU T-cycles mod 8192; step index 0-7 advances every 8192 cycles
    fs_cycles: u32,             // accumulator; when >= 8192, increment fs_step, subtract 8192
    fs_step: u8,                // 0-7, wraps at 8

    // Sample accumulator: fractional cycle budget for sample generation
    // Represented as a fixed-point counter. Each CPU cycle adds 1; when >= cycles_per_sample
    // (which is sample_period_int + fractional carry), emit a sample.
    sample_cycles: f32,         // accumulated T-cycles toward next sample
    sample_period: f32,         // T-cycles per sample = 4194304.0 / sample_rate
}
```

`Apu::new(sample_rate: f32) -> Apu` initializes all fields; `sample_period = 4194304.0 / sample_rate`.

#### `Ch1` — Pulse + Sweep

```rust
pub struct Ch1 {
    // NR10: sweep
    pub sweep_period: u8,       // bits 6-4 (0-7); 0 = sweep disabled
    pub sweep_negate: bool,     // bit 3
    pub sweep_shift: u8,        // bits 2-0

    // NR11: duty/length
    pub duty: u8,               // bits 7-6 (0-3 → wave patterns)
    pub length_load: u8,        // bits 5-0, written value; actual counter = 64 - length_load

    // NR12: envelope
    pub env_initial: u8,        // bits 7-4 (0-15)
    pub env_add: bool,          // bit 3 (true = add/increase)
    pub env_period: u8,         // bits 2-0 (0 = disabled)

    // NR13/NR14: frequency low/high
    pub freq_low: u8,           // NR13 (bits 7-0 of 11-bit frequency)
    pub freq_high: u8,          // NR14 bits 2-0 (bits 10-8 of 11-bit frequency)
    pub length_enable: bool,    // NR14 bit 6
    // NR14 bit 7 is trigger-only (write-only strobe, not stored)

    // Runtime state
    pub enabled: bool,          // channel DAC + length gate
    freq_timer: u16,            // counts down in T-cycles; reload = (2048 - freq) * 4
    duty_pos: u8,               // 0-7, index into duty waveform
    length_counter: u8,         // counts down from (64 - length_load); disables ch when 0
    env_volume: u8,             // current volume 0-15
    env_timer: u8,              // counts down; reload from env_period
    sweep_timer: u8,            // counts down; reload from sweep_period (treat 0 as 8)
    sweep_shadow: u16,          // shadow copy of 11-bit frequency used by sweep calc
    sweep_enabled: bool,        // internal sweep enabled flag (set on trigger)
}
```

#### `Ch2` — Pulse (no sweep)

```rust
pub struct Ch2 {
    pub duty: u8,
    pub length_load: u8,
    pub env_initial: u8,
    pub env_add: bool,
    pub env_period: u8,
    pub freq_low: u8,
    pub freq_high: u8,
    pub length_enable: bool,

    pub enabled: bool,
    freq_timer: u16,
    duty_pos: u8,
    length_counter: u8,
    env_volume: u8,
    env_timer: u8,
}
```

#### `Ch3` — Wave

```rust
pub struct Ch3 {
    // NR30: DAC power (bit 7)
    pub dac_on: bool,
    // NR31: length load (8-bit; counter = 256 - length_load)
    pub length_load: u8,
    // NR32: output level (bits 6-5: 0=mute, 1=100%, 2=50%, 3=25%)
    pub output_level: u8,
    // NR33/NR34: frequency
    pub freq_low: u8,
    pub freq_high: u8,          // bits 2-0
    pub length_enable: bool,    // NR34 bit 6

    pub enabled: bool,
    freq_timer: u16,            // reload = (2048 - freq) * 2  (wave is clocked at 2× freq)
    wave_pos: u8,               // 0-31, index into wave RAM nibbles
    length_counter: u16,        // counts down from (256 - length_load)
    wave_ram: [u8; 16],         // 0xFF30–0xFF3F; each byte holds 2 nibbles
}
```

#### `Ch4` — Noise

```rust
pub struct Ch4 {
    // NR41: length load (bits 5-0; counter = 64 - length_load)
    pub length_load: u8,
    // NR42: envelope
    pub env_initial: u8,
    pub env_add: bool,
    pub env_period: u8,
    // NR43: polynomial counter
    pub clock_shift: u8,        // bits 7-4
    pub lfsr_width: bool,       // bit 3: false=15-bit, true=7-bit
    pub divisor_code: u8,       // bits 2-0 (0→div=8, 1→16, 2→32, ..., 7→128; see table)
    // NR44: length enable / trigger
    pub length_enable: bool,    // bit 6

    pub enabled: bool,
    freq_timer: u16,            // counts down in T-cycles; reload = divisor << clock_shift
    lfsr: u16,                  // 15-bit LFSR state (only bits 14-0 used)
    length_counter: u8,
    env_volume: u8,
    env_timer: u8,
}
```

CH4 divisor table (from `divisor_code`):
```
code: 0 → 8
code: 1 → 16
code: 2 → 32
code: 3 → 48
code: 4 → 64
code: 5 → 80
code: 6 → 96
code: 7 → 112
```
Reload formula: `freq_timer = (divisor as u16) << (clock_shift as u16)`. Minimum value is 8 (when code=0, shift=0).

### Key Decisions

**`ScriptProcessorNode` over `AudioWorklet`.**
`AudioWorklet` requires the audio processor to run in a separate `AudioWorkletGlobalScope`. Sharing the WASM module with that scope requires either: (a) passing a `SharedArrayBuffer` between the main thread and the worklet, which requires `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` HTTP response headers, or (b) a bundler-based setup. Neither is available on a plain GitHub Pages deployment (GitHub Pages serves static files with no custom headers). `ScriptProcessorNode` runs its callback on the main thread where the WASM module already lives, requiring no shared memory or special headers. It is deprecated but universally supported and will not be removed imminently from browsers. A comment in `index.js` acknowledges the deprecation and documents the constraint.

**Audio callback as master clock.**
When rAF drives emulation, the emulator runs ~1 frame worth of cycles per rAF tick. The browser can throttle rAF (hidden tabs, power saving), jitter its timing, or drop frames entirely. The audio subsystem has its own hardware-backed clock and will call `onaudioprocess` on a predictable schedule regardless of tab visibility. By running exactly the right number of CPU cycles per audio callback, audio output and CPU timing are kept in lockstep. Video rendering happens inside the same callback, so it is never faster or slower than the audio clock.

**`bufferSize = 4096`.**
At 44100 Hz, a 4096-sample buffer fires approximately every 92.9 ms (~10.7 times/second). A buffer of 2048 fires ~21.5 times/second but doubles callback overhead and increases the risk of under-runs on slow machines. 4096 provides a comfortable margin with negligible perceptible latency for an emulator. The Game Boy itself runs at ~59.7 fps; the 10.7 Hz video update rate from 4096-sample buffers means roughly 1 video frame rendered per 5-6 Game Boy frames — acceptable because the canvas simply shows the latest completed frame each time the callback fires.

**Functional accuracy, not cycle-accurate.**
Cycle-accurate APU emulation requires tracking individual sample positions against T-cycle boundaries for every channel trigger and length counter tick. This is necessary for correct behavior in games that exploit specific timing (e.g. certain sound effects in Prehistorik Man). For the games targeted by this phase (Tetris, Pokemon), functional accuracy — correct register semantics, correct frequencies, correct envelopes — is sufficient. Edge cases like the "zombie mode" envelope behavior, power-on state of the wave RAM, and exact behavior when length counter is 0 on trigger are not required for this phase.

**Partial-cycle accumulation via `f32` accumulator.**
At 44100 Hz, one sample requires `4194304 / 44100 ≈ 95.1` CPU T-cycles. This is not an integer. The `Apu` struct tracks a `sample_cycles: f32` accumulator that increments by the number of T-cycles processed on each `step_apu` call. When `sample_cycles >= sample_period`, a sample is emitted and `sample_period` is subtracted (not zeroed) so fractional carry accumulates correctly over time. Over 1 second this produces exactly 44100 samples with negligible floating-point drift.

**`step_samples` as the new emulator entry point.**
`Emulator::step_frame()` is retained unchanged (used by existing tests). A new `step_samples(n: usize) -> Vec<f32>` method runs the CPU/PPU/APU in lockstep, collecting APU output until `n` stereo pairs are accumulated. The WASM export `step_samples` wraps this method.

**APU initialized with `sample_rate = 44100.0`.**
The sample rate is fixed at 44100 Hz. The `AudioContext` in JS is created with `{ sampleRate: 44100 }` to ensure the `onaudioprocess` buffer always arrives at 44100 Hz, matching the APU's internal accumulator. This avoids resampling.

**Channel output range.**
Each channel produces a value in `[0.0, 1.0]` before panning/volume. After mixing, each output channel (left/right) is the sum of up to 4 channel outputs, scaled by master volume. The master volume scale maps the 3-bit NR50 volume field (0–7) to `(vol + 1) / 8` so volume=7 gives 1.0 and volume=0 gives 0.125 (not silence — NR50 volume=0 is not "mute"). The sum of 4 channels at max volume is `4 * 1.0 = 4.0`; after the `/ 4` normalization the range is `[0.0, 1.0]`. Each side is then multiplied by its NR50 master volume scalar. Final output is in `[-0.0, 1.0]` (all Game Boy audio is non-negative before the DC-offset filter; a DC-offset subtraction is intentionally omitted as a functional-accuracy tradeoff).

### Sample Mixing Formula

For each sample pair, compute:

```
ch1_out = if ch1.enabled { duty_sample(ch1) * (ch1.env_volume as f32 / 15.0) } else { 0.0 }
ch2_out = if ch2.enabled { duty_sample(ch2) * (ch2.env_volume as f32 / 15.0) } else { 0.0 }
ch3_out = if ch3.enabled && ch3.dac_on { wave_sample(ch3) } else { 0.0 }
ch4_out = if ch4.enabled { noise_sample(ch4) * (ch4.env_volume as f32 / 15.0) } else { 0.0 }

left_vol_scale  = ((nr50 >> 4) & 0x07) as f32 + 1.0) / 8.0
right_vol_scale = ((nr50)      & 0x07) as f32 + 1.0) / 8.0

left  = (ch1_out * pan(nr51, CH1_LEFT)  + ch2_out * pan(nr51, CH2_LEFT)
       + ch3_out * pan(nr51, CH3_LEFT)  + ch4_out * pan(nr51, CH4_LEFT)) / 4.0 * left_vol_scale
right = (ch1_out * pan(nr51, CH1_RIGHT) + ch2_out * pan(nr51, CH2_RIGHT)
       + ch3_out * pan(nr51, CH3_RIGHT) + ch4_out * pan(nr51, CH4_RIGHT)) / 4.0 * right_vol_scale
```

`pan(nr51, bit)` returns `1.0` if the panning bit is set, `0.0` otherwise. NR51 bit positions:
- CH1-right: bit 0, CH2-right: bit 1, CH3-right: bit 2, CH4-right: bit 3
- CH1-left: bit 4, CH2-left: bit 5, CH3-left: bit 6, CH4-left: bit 7

`duty_sample(ch)` returns `1.0` if the current duty waveform bit is 1, else `0.0`. Duty waveforms (indexed by `duty` field 0–3):
```
0 → [0,0,0,0,0,0,0,1]  (12.5%)
1 → [1,0,0,0,0,0,0,1]  (25%)
2 → [1,0,0,0,1,1,1,1]  (50%)
3 → [0,1,1,1,1,1,1,0]  (75%)
```
Index into this array with `duty_pos` (0–7). The waveform advances when `freq_timer` counts down to zero.

`wave_sample(ch3)` reads nibble `wave_pos` from wave RAM, shifts by `output_level`, and converts to `[0.0, 1.0]`. Specifically:
```
raw_nibble = if wave_pos % 2 == 0 { wave_ram[wave_pos/2] >> 4 } else { wave_ram[wave_pos/2] & 0x0F }
shifted    = match output_level { 0 => 0, 1 => raw_nibble, 2 => raw_nibble >> 1, 3 => raw_nibble >> 2, _ => 0 }
sample     = shifted as f32 / 15.0
```

`noise_sample(ch4)` returns `1.0` if LFSR bit 0 is 0 (inverted), else `0.0`. The LFSR advances when `freq_timer` counts down to zero:
```
xor_bit = (lfsr & 1) ^ ((lfsr >> 1) & 1)
lfsr = (lfsr >> 1) | (xor_bit << 14)
if lfsr_width { lfsr = (lfsr & !(1 << 6)) | (xor_bit << 6) }  // 7-bit mode
```

### Frame Sequencer Tick Schedule

```
fs_step | length | sweep | envelope
   0    |   yes  |  no   |   no
   1    |   no   |  no   |   no
   2    |   yes  |  yes  |   no
   3    |   no   |  no   |   no
   4    |   yes  |  no   |   no
   5    |   no   |  no   |   no
   6    |   yes  |  yes  |   no
   7    |   no   |  no   |   yes
```

### Frequency Timer Clocking

- **CH1 / CH2:** `freq_timer` decrements by 1 each T-cycle. On reaching 0: reload `freq_timer = (2048 - freq11()) * 4` where `freq11()` = `(freq_high as u16 & 0x07) << 8 | freq_low as u16`; advance `duty_pos = (duty_pos + 1) % 8`.
- **CH3:** `freq_timer` decrements by 1 each T-cycle. On reaching 0: reload `freq_timer = (2048 - freq11()) * 2`; advance `wave_pos = (wave_pos + 1) % 32`.
- **CH4:** `freq_timer` decrements by 1 each T-cycle. On reaching 0: reload using divisor table; clock LFSR.

In practice, decrementing `freq_timer` one T-cycle at a time in a hot loop is inefficient. In `step_apu`, compute `min_until_next = freq_timer` and advance by bulk T-cycles up to that point. The spec uses a straightforward tick-at-a-time model for clarity; the implementation may use the bulk approach for performance as long as the observable output is identical.

### Trigger Behavior (NRx4 bit 7 write)

When any channel's trigger bit is written:
1. Set `channel.enabled = true`.
2. If `length_counter == 0`, reload it: CH1/CH2/CH4: `length_counter = 64`; CH3: `length_counter = 256`.
3. Reload `freq_timer` from current frequency registers.
4. CH1/CH2/CH4: reload `env_volume = env_initial`, `env_timer = env_period` (treat 0 as 8).
5. CH3: reset `wave_pos = 0`.
6. CH4: reload LFSR to `0x7FFF`.
7. CH1 only: reload `sweep_shadow = freq11()`, `sweep_timer = sweep_period` (treat 0 as 8), `sweep_enabled = (sweep_period > 0 || sweep_shift > 0)`. Immediately perform one overflow check (but do NOT update frequency on this first check — only check for overflow).
8. If the channel's DAC is off (NR12 bits 7-3 = 0 for CH1/CH2/CH4; NR30 bit 7 = 0 for CH3), disable the channel immediately after triggering.

### Sweep Calculation (CH1)

When the sweep unit ticks (fs_step 2 or 6) and `sweep_enabled` is true:
```
if sweep_timer > 0 { sweep_timer -= 1; }
if sweep_timer == 0 {
    sweep_timer = if sweep_period == 0 { 8 } else { sweep_period };
    if sweep_enabled && sweep_period > 0 {
        new_freq = sweep_calc();
        if new_freq <= 0x7FF && sweep_shift > 0 {
            sweep_shadow = new_freq;
            ch1.freq_low  = (new_freq & 0xFF) as u8;
            ch1.freq_high = (ch1.freq_high & !0x07) | ((new_freq >> 8) as u8 & 0x07);
            sweep_calc();  // second overflow check only, no write
        }
    }
}
```
`sweep_calc()`:
```
delta = sweep_shadow >> sweep_shift;
new_freq = if sweep_negate { sweep_shadow - delta } else { sweep_shadow + delta };
if new_freq > 0x7FF { ch1.enabled = false; }
new_freq
```

### `Emulator::step_samples`

```rust
pub fn step_samples(&mut self, n: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n * 2);
    while out.len() < n * 2 {
        let t = self.cpu.step(&mut self.bus);
        self.bus.step_timer(t);
        self.bus.step_ppu(t);
        self.bus.step_apu(t, &mut out);
    }
    out.truncate(n * 2);
    out
}
```

`Bus::step_apu(t_cycles: u32, out: &mut Vec<f32>)` advances the APU by `t_cycles` T-cycles, pushing `[L, R]` pairs onto `out` for each sample generated during those cycles.

### WASM export `step_samples`

Add to `crates/gpuboy-wasm/src/lib.rs`:

```rust
#[wasm_bindgen]
pub fn step_samples(n: usize) -> Vec<f32> {
    EMULATOR.with(|e| {
        if let Some(emu) = e.borrow_mut().as_mut() {
            emu.step_samples(n)
        } else {
            vec![0.0f32; n * 2]
        }
    })
}
```

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
    // AudioWorklet would require those headers; see phase-5 spec Key Decisions.
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

### `Bus` changes

Add `apu: Apu` field to `Bus`. Route APU registers in `read` and `write`:

In `Bus::new`:
```rust
apu: Apu::new(44100.0),
```

In `Bus::read`, add these arms before the catch-all `_ => 0xFF`:
```rust
0xFF10 => self.apu.read(0xFF10),
// ... (all APU registers 0xFF10–0xFF3F route to self.apu.read(addr))
0xFF10..=0xFF3F => self.apu.read(addr),
```

In `Bus::write`, add:
```rust
0xFF10..=0xFF3F => self.apu.write(addr, val),
```

Add `Bus::step_apu`:
```rust
pub fn step_apu(&mut self, t_cycles: u32, out: &mut Vec<f32>) {
    self.apu.step(t_cycles, out);
}
```

### `Apu` register map

```
0xFF10  NR10  CH1 sweep:     bits 6-4=period, bit 3=negate, bits 2-0=shift
0xFF11  NR11  CH1 duty/len:  bits 7-6=duty, bits 5-0=length_load
0xFF12  NR12  CH1 envelope:  bits 7-4=initial, bit 3=add, bits 2-0=period
0xFF13  NR13  CH1 freq low:  bits 7-0
0xFF14  NR14  CH1 ctrl:      bit 7=trigger(w), bit 6=length_enable, bits 2-0=freq high

0xFF15  ---   unused (reads 0xFF)
0xFF16  NR21  CH2 duty/len
0xFF17  NR22  CH2 envelope
0xFF18  NR23  CH2 freq low
0xFF19  NR24  CH2 ctrl

0xFF1A  NR30  CH3 DAC:       bit 7=dac_on
0xFF1B  NR31  CH3 length
0xFF1C  NR32  CH3 output:    bits 6-5=output_level
0xFF1D  NR33  CH3 freq low
0xFF1E  NR34  CH3 ctrl

0xFF1F  ---   unused
0xFF20  NR41  CH4 length:    bits 5-0=length_load
0xFF21  NR42  CH4 envelope
0xFF22  NR43  CH4 poly:      bits 7-4=clock_shift, bit 3=lfsr_width, bits 2-0=divisor_code
0xFF23  NR44  CH4 ctrl:      bit 7=trigger(w), bit 6=length_enable

0xFF24  NR50  master volume: bits 6-4=left vol, bits 2-0=right vol
0xFF25  NR51  panning
0xFF26  NR52  sound enable:  bit 7=all_on(r/w), bits 3-0=ch_on(read-only)

0xFF27–0xFF2F  unused
0xFF30–0xFF3F  wave RAM (16 bytes)
```

**Read behavior:**
- NR52 (0xFF26): bit 7 = `enabled`; bits 3-0 = ch4.enabled | ch3.enabled<<2 | ch2.enabled<<1 | ch1.enabled (read-only status). Bits 6-4 always read 1. Unused register addresses read 0xFF.
- NR11/NR21 (write has both duty and length; read returns only duty in bits 7-6, lower bits read as 1).
- NR13/NR23/NR33 (freq low): write-only, reads return 0xFF.
- NR14/NR24/NR34/NR44 bit 7 (trigger): write-only; reads return the length_enable bit in bit 6, other bits read 1.

**Write behavior when APU is disabled (NR52 bit 7 = 0):**
All register writes are ignored except NR52 itself and (on DMG) length counter writes. For simplicity in this functional implementation: when `enabled == false`, ignore all writes except to NR52 (0xFF26).

## Tasks

- [ ] 1. Create `crates/gpuboy-core/src/apu.rs`. Define the `Apu`, `Ch1`, `Ch2`, `Ch3`, `Ch4` structs exactly as specified in §Data Structures. Add `use crate::apu::Apu;` to `crates/gpuboy-core/src/lib.rs` and add `pub mod apu;`. *(req 1)*

- [ ] 2. Implement `Apu::new(sample_rate: f32) -> Apu`. Initialize: `enabled = false`, `nr50 = 0x77` (power-on default: both sides max volume), `nr51 = 0xF3` (power-on default panning), `fs_cycles = 0`, `fs_step = 0`, `sample_cycles = 0.0`, `sample_period = 4194304.0 / sample_rate`. Initialize all four channel structs to zero/false/default (all disabled). For Ch4, initialize `lfsr = 0x7FFF`. *(req 2, 3)*

- [ ] 3. Implement `Apu::read(&self, addr: u16) -> u8` following the register map in §`Apu` register map. Route wave RAM reads (0xFF30–0xFF3F) to `self.ch3.wave_ram[(addr - 0xFF30) as usize]`. Unused registers return 0xFF. NR52 read: `0x70 | (self.enabled as u8) << 7 | (self.ch4.enabled as u8) << 3 | (self.ch3.enabled as u8) << 2 | (self.ch2.enabled as u8) << 1 | (self.ch1.enabled as u8)`. *(req 1, 9)*

- [ ] 4. Implement `Apu::write(&mut self, addr: u16, val: u8)`. When `!self.enabled && addr != 0xFF26`, return immediately. Route each address to its register field per §`Apu` register map. For NRx4 trigger writes (bit 7 set): call the appropriate trigger handler. For NR52 write: if bit 7 goes from 1→0, clear all channel enabled flags and reset all channel runtime state (but not register values); if bit 7 goes from 0→1, set `enabled = true`. Route wave RAM writes (0xFF30–0xFF3F) to `self.ch3.wave_ram`. *(req 1, 2, 5, 7)*

- [ ] 5. Implement trigger handlers as `Apu::trigger_ch1`, `trigger_ch2`, `trigger_ch3`, `trigger_ch4` (private methods). Follow the 8-step trigger behavior in §Trigger Behavior exactly. For CH1 sweep: after loading `sweep_shadow` and `sweep_timer`, call `apu_sweep_check` (performs overflow check only, no write) as step 7. *(req 5, 7)*

- [ ] 6. Implement `Apu::step(&mut self, t_cycles: u32, out: &mut Vec<f32>)`. This is the main APU clock. Process `t_cycles` T-cycles in a loop. For each T-cycle:
    - Decrement `ch1.freq_timer`, `ch2.freq_timer`, `ch3.freq_timer`, `ch4.freq_timer` by 1. On reaching 0: reload and advance duty/wave/LFSR per §Frequency Timer Clocking.
    - Increment `fs_cycles`. When `fs_cycles >= 8192`: subtract 8192, call `tick_frame_sequencer()`.
    - Increment `sample_cycles` by 1.0. When `sample_cycles >= sample_period`: subtract `sample_period`, call `mix_sample(out)`.
    If `!self.enabled`, skip all channel updates and push `[0.0, 0.0]` when the sample threshold is crossed. *(req 2, 3, 4, 6, 8)*

- [ ] 7. Implement `Apu::tick_frame_sequencer(&mut self)` following the schedule in §Frame Sequencer Tick Schedule. Tick length counters (CH1, CH2, CH3, CH4) at steps 0,2,4,6: if `length_enable && length_counter > 0 { length_counter -= 1; if length_counter == 0 { enabled = false; } }`. Tick CH1 sweep at steps 2,6: call `tick_sweep()`. Tick envelopes (CH1, CH2, CH4) at step 7: for each: if `env_period > 0 { env_timer -= 1; if env_timer == 0 { env_timer = env_period; if env_add && env_volume < 15 { env_volume += 1; } else if !env_add && env_volume > 0 { env_volume -= 1; } } }`. Advance `fs_step = (fs_step + 1) % 8`. *(req 4, 6, 8)*

- [ ] 8. Implement `Apu::tick_sweep(&mut self)` following §Sweep Calculation exactly. *(req 6)*

- [ ] 9. Implement `Apu::mix_sample(&self, out: &mut Vec<f32>)` following §Sample Mixing Formula. Compute `ch1_out`, `ch2_out`, `ch3_out`, `ch4_out` using `duty_sample`, `wave_sample`, `noise_sample` helpers. Apply NR51 panning bits, divide by 4, apply NR50 master volume scales. Push `left` then `right` onto `out`. *(req 3, 9)*

- [ ] 10. Implement the three sample helpers as private methods or free functions in `apu.rs`:
    - `duty_sample(duty: u8, pos: u8) -> f32` — index into the 4×8 duty table with `(duty & 3) as usize` and `(pos & 7) as usize`; return 1.0 or 0.0.
    - `wave_sample(wave_ram: &[u8; 16], pos: u8, output_level: u8) -> f32` — extract nibble, shift, normalize per §Data Structures `wave_sample` formula.
    - `noise_sample(lfsr: u16) -> f32` — return `if lfsr & 1 == 0 { 1.0 } else { 0.0 }`. *(req 3)*

- [ ] 11. Add `pub apu: Apu` to the `Bus` struct in `crates/gpuboy-core/src/bus.rs`. In `Bus::new`, add `apu: Apu::new(44100.0)`. In `Bus::read`, add the arm `0xFF10..=0xFF3F => self.apu.read(addr)` before the catch-all (insert before the `_ => 0xFF` arm). In `Bus::write`, add `0xFF10..=0xFF3F => self.apu.write(addr, val)` before the catch-all. Add `pub fn step_apu(&mut self, t_cycles: u32, out: &mut Vec<f32>) { self.apu.step(t_cycles, out); }`. Add `use crate::apu::Apu;` at the top of `bus.rs`. *(req 1)*

- [ ] 12. Add `Emulator::step_samples` to `crates/gpuboy-core/src/lib.rs` following §`Emulator::step_samples` exactly. Keep `step_frame` unchanged. *(req 3, 11)*

- [ ] 13. Add the `step_samples` WASM export to `crates/gpuboy-wasm/src/lib.rs` following §WASM export `step_samples`. Keep all existing exports unchanged. *(req 11)*

- [ ] 14. Update `www/index.js` following §JS changes. Replace the rAF loop with the `ScriptProcessorNode` approach. Remove the `loop()` function and `animationId` variable. Add the `step_samples` import. Keep `render2d`, `render_frame_wgpu`, all renderer toggle logic, and the ROM picker logic (adapted to call `startAudio()` instead of `loop()`). *(req 10, 11, 12)*

- [ ] 15. Verify `www/index.html` requires no changes: the HTML from Phase 3b already has both canvases, the toggle button, and the error div. No modifications needed.

- [ ] 16. Add a basic smoke-test in `crates/gpuboy-core/src/apu.rs` under `#[cfg(test)]`:
    ```rust
    #[test]
    fn apu_generates_samples() {
        let mut apu = Apu::new(44100.0);
        apu.enabled = true;
        apu.nr50 = 0x77;
        apu.nr51 = 0xFF;
        // Enable CH1 with a simple tone: duty=2 (50%), no length, no envelope decay
        apu.write(0xFF11, 0x80); // duty=2, length=0
        apu.write(0xFF12, 0xF0); // env initial=15, add=false, period=0 (no decay)
        apu.write(0xFF13, 0x00); // freq low
        apu.write(0xFF14, 0x87); // trigger + freq high = 7 → freq = 0x700 = 1792
        let mut out = Vec::new();
        // Step enough cycles to generate at least 100 samples (100 * ~95 = 9500 cycles)
        apu.step(10000, &mut out);
        assert!(out.len() >= 200, "expected at least 100 stereo pairs");
        // With CH1 enabled at volume 15, some samples should be non-zero
        assert!(out.iter().any(|&s| s > 0.0), "expected non-zero samples from CH1");
    }
    ```
    *(req 3)*

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
