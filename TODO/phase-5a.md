# Phase 5a: APU Core

## Overview

Implements the Game Boy Audio Processing Unit (APU) entirely in Rust inside `gpuboy-core`. Covers all four channels — CH1 (pulse + sweep), CH2 (pulse), CH3 (wave), CH4 (noise) — plus the frame sequencer, volume envelopes, sweep unit, and sample mixer. Adds `Emulator::step_samples(n) -> Vec<f32>` as the new emulation entry point used by the audio clock in Phase 5b. No WASM exports or JS changes in this phase. Green light is verified by unit tests and `cargo test` passing clean.

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

## Acceptance Criteria

- [ ] `cargo test -p gpuboy-core` passes with all existing tests plus the new `apu_generates_samples` smoke test.
- [ ] The smoke test confirms `step_samples` produces at least 100 stereo pairs from a CH1-enabled APU with 10 000 T-cycles of input, and at least one sample is non-zero.
- [ ] `cargo clippy -- -D warnings` passes clean.

## Design

### Architecture

```
crates/gpuboy-core/src/
  apu.rs     — NEW: Apu, Ch1, Ch2, Ch3, Ch4 structs + all logic
  bus.rs     — add apu: Apu field; route 0xFF10–0xFF3F; add step_apu
  lib.rs     — add pub mod apu; add Emulator::step_samples
```

No other crates change in this phase.

### Data Structures

#### `Apu`

```rust
pub struct Apu {
    pub enabled: bool,          // NR52 bit 7

    pub ch1: Ch1,
    pub ch2: Ch2,
    pub ch3: Ch3,
    pub ch4: Ch4,

    pub nr50: u8,               // bits 6-4 = left vol 0-7, bits 2-0 = right vol 0-7
    pub nr51: u8,               // bit 7=CH4-L, 6=CH3-L, 5=CH2-L, 4=CH1-L,
                                //              3=CH4-R, 2=CH3-R, 1=CH2-R, 0=CH1-R

    fs_cycles: u32,             // accumulator; when >= 8192, increment fs_step, subtract 8192
    fs_step: u8,                // 0-7, wraps at 8

    sample_cycles: f32,         // accumulated T-cycles toward next sample
    sample_period: f32,         // T-cycles per sample = 4194304.0 / sample_rate
}
```

`Apu::new(sample_rate: f32) -> Apu` initializes all fields; `sample_period = 4194304.0 / sample_rate`.

#### `Ch1` — Pulse + Sweep

```rust
pub struct Ch1 {
    pub sweep_period: u8,
    pub sweep_negate: bool,
    pub sweep_shift: u8,

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
    sweep_timer: u8,
    sweep_shadow: u16,
    sweep_enabled: bool,
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
    pub dac_on: bool,
    pub length_load: u8,
    pub output_level: u8,       // bits 6-5: 0=mute, 1=100%, 2=50%, 3=25%
    pub freq_low: u8,
    pub freq_high: u8,
    pub length_enable: bool,

    pub enabled: bool,
    freq_timer: u16,            // reload = (2048 - freq) * 2
    wave_pos: u8,               // 0-31, index into nibbles
    length_counter: u16,        // counts down from (256 - length_load)
    wave_ram: [u8; 16],         // 0xFF30–0xFF3F
}
```

#### `Ch4` — Noise

```rust
pub struct Ch4 {
    pub length_load: u8,
    pub env_initial: u8,
    pub env_add: bool,
    pub env_period: u8,
    pub clock_shift: u8,        // bits 7-4
    pub lfsr_width: bool,       // bit 3: false=15-bit, true=7-bit
    pub divisor_code: u8,       // bits 2-0
    pub length_enable: bool,

    pub enabled: bool,
    freq_timer: u16,
    lfsr: u16,                  // 15-bit LFSR; initialize to 0x7FFF
    length_counter: u8,
    env_volume: u8,
    env_timer: u8,
}
```

CH4 divisor table: code 0→8, 1→16, 2→32, 3→48, 4→64, 5→80, 6→96, 7→112.
Reload formula: `freq_timer = (divisor as u16) << (clock_shift as u16)`.

### `Apu` register map

```
0xFF10  NR10  CH1 sweep:     bits 6-4=period, bit 3=negate, bits 2-0=shift
0xFF11  NR11  CH1 duty/len:  bits 7-6=duty, bits 5-0=length_load
0xFF12  NR12  CH1 envelope:  bits 7-4=initial, bit 3=add, bits 2-0=period
0xFF13  NR13  CH1 freq low   (write-only; reads 0xFF)
0xFF14  NR14  CH1 ctrl:      bit 7=trigger(w), bit 6=length_enable, bits 2-0=freq high

0xFF15  ---   unused (reads 0xFF)
0xFF16  NR21  CH2 duty/len
0xFF17  NR22  CH2 envelope
0xFF18  NR23  CH2 freq low   (write-only; reads 0xFF)
0xFF19  NR24  CH2 ctrl

0xFF1A  NR30  CH3 DAC:       bit 7=dac_on
0xFF1B  NR31  CH3 length
0xFF1C  NR32  CH3 output:    bits 6-5=output_level
0xFF1D  NR33  CH3 freq low   (write-only; reads 0xFF)
0xFF1E  NR34  CH3 ctrl

0xFF1F  ---   unused
0xFF20  NR41  CH4 length:    bits 5-0=length_load
0xFF21  NR42  CH4 envelope
0xFF22  NR43  CH4 poly:      bits 7-4=clock_shift, bit 3=lfsr_width, bits 2-0=divisor_code
0xFF23  NR44  CH4 ctrl:      bit 7=trigger(w), bit 6=length_enable

0xFF24  NR50  master volume: bits 6-4=left vol, bits 2-0=right vol
0xFF25  NR51  panning
0xFF26  NR52  sound enable:  bit 7=all_on(r/w), bits 3-0=ch_on(read-only)

0xFF27–0xFF2F  unused (reads 0xFF)
0xFF30–0xFF3F  wave RAM (16 bytes)
```

**Read behavior:**
- NR52: `0x70 | (enabled as u8) << 7 | ch4.enabled<<3 | ch3.enabled<<2 | ch2.enabled<<1 | ch1.enabled`
- NR11/NR21: reads return duty in bits 7-6; lower 6 bits read as 1.
- NR13/NR23/NR33: write-only; reads return 0xFF.
- NR14/NR24/NR34/NR44 bit 7: write-only; reads return length_enable in bit 6, other bits 1.
- Unused addresses: return 0xFF.

**Write behavior when APU disabled:** when `enabled == false`, ignore all writes except to NR52 (0xFF26).

**NR52 write:** if bit 7 goes 1→0, clear all channel `enabled` flags and reset runtime state (leave register values intact); if 0→1, set `enabled = true`.

### `Cartridge::read` — Frequency Timer Clocking

- **CH1 / CH2:** decrement `freq_timer` by 1 per T-cycle. On 0: reload `freq_timer = (2048 - freq11()) * 4`; advance `duty_pos = (duty_pos + 1) % 8`.
- **CH3:** decrement `freq_timer` by 1 per T-cycle. On 0: reload `freq_timer = (2048 - freq11()) * 2`; advance `wave_pos = (wave_pos + 1) % 32`.
- **CH4:** decrement `freq_timer` by 1 per T-cycle. On 0: reload using divisor table; clock LFSR.

### Trigger Behavior (NRx4 bit 7 write)

1. Set `channel.enabled = true`.
2. If `length_counter == 0`: reload CH1/CH2/CH4 to 64; CH3 to 256.
3. Reload `freq_timer` from current frequency registers.
4. CH1/CH2/CH4: reload `env_volume = env_initial`, `env_timer = env_period` (treat 0 as 8).
5. CH3: reset `wave_pos = 0`.
6. CH4: reload `lfsr = 0x7FFF`.
7. CH1 only: reload `sweep_shadow = freq11()`, `sweep_timer = if sweep_period == 0 { 8 } else { sweep_period }`, `sweep_enabled = sweep_period > 0 || sweep_shift > 0`. Perform one overflow check (no frequency write on this first check).
8. If channel DAC is off (NR12 bits 7-3 = 0 for CH1/CH2/CH4; NR30 bit 7 = 0 for CH3), disable the channel immediately.

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

Length tick: `if length_enable && length_counter > 0 { length_counter -= 1; if length_counter == 0 { enabled = false; } }`.

Envelope tick (CH1, CH2, CH4): `if env_period > 0 { env_timer -= 1; if env_timer == 0 { env_timer = env_period; if env_add && env_volume < 15 { env_volume += 1; } else if !env_add && env_volume > 0 { env_volume -= 1; } } }`.

### Sweep Calculation (CH1)

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

### Sample Mixing Formula

```
ch1_out = if ch1.enabled { duty_sample(ch1) * (ch1.env_volume as f32 / 15.0) } else { 0.0 }
ch2_out = if ch2.enabled { duty_sample(ch2) * (ch2.env_volume as f32 / 15.0) } else { 0.0 }
ch3_out = if ch3.enabled && ch3.dac_on { wave_sample(ch3) } else { 0.0 }
ch4_out = if ch4.enabled { noise_sample(ch4) * (ch4.env_volume as f32 / 15.0) } else { 0.0 }

left_vol_scale  = (((nr50 >> 4) & 0x07) as f32 + 1.0) / 8.0
right_vol_scale = ((nr50 & 0x07) as f32 + 1.0) / 8.0

left  = (ch1_out * pan(nr51,4) + ch2_out * pan(nr51,5)
       + ch3_out * pan(nr51,6) + ch4_out * pan(nr51,7)) / 4.0 * left_vol_scale
right = (ch1_out * pan(nr51,0) + ch2_out * pan(nr51,1)
       + ch3_out * pan(nr51,2) + ch4_out * pan(nr51,3)) / 4.0 * right_vol_scale
```

`pan(nr51, bit)` = `if (nr51 >> bit) & 1 == 1 { 1.0 } else { 0.0 }`.

Duty waveforms (indexed 0–3, 8 bits each):
```
0 → [0,0,0,0,0,0,0,1]  (12.5%)
1 → [1,0,0,0,0,0,0,1]  (25%)
2 → [1,0,0,0,1,1,1,1]  (50%)
3 → [0,1,1,1,1,1,1,0]  (75%)
```

`wave_sample`: `raw = if wave_pos%2==0 { wave_ram[wave_pos/2]>>4 } else { wave_ram[wave_pos/2]&0x0F }`. Shift: `match output_level { 0=>0, 1=>raw, 2=>raw>>1, 3=>raw>>2, _=>0 }`. Normalize: `shifted as f32 / 15.0`.

`noise_sample`: returns `if lfsr & 1 == 0 { 1.0 } else { 0.0 }`. LFSR advance: `let xor = (lfsr&1)^((lfsr>>1)&1); lfsr = (lfsr>>1)|(xor<<14); if lfsr_width { lfsr=(lfsr&!(1<<6))|(xor<<6); }`.

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

`Bus::step_apu(t_cycles: u32, out: &mut Vec<f32>)` calls `self.apu.step(t_cycles, out)`.

### Key Decisions

**`ScriptProcessorNode` deferred to 5b.** APU core is pure Rust logic. Phase 5a is verified entirely through unit tests and `cargo test`. The `step_samples` method on `Emulator` is the contract 5b depends on.

**Partial-cycle accumulation via `f32`.** At 44100 Hz one sample ≈ 95.1 T-cycles. `sample_cycles` accumulates and when `>= sample_period` a sample is emitted and `sample_period` is subtracted (not zeroed) so fractional carry accumulates correctly.

**Functional accuracy, not cycle-accurate.** Edge cases like zombie-mode envelope behavior and power-on wave RAM state are not required for this phase. Correct register semantics, frequencies, envelopes, and sweep are sufficient.

## Tasks

- [ ] 1. Create `crates/gpuboy-core/src/apu.rs`. Define `Apu`, `Ch1`, `Ch2`, `Ch3`, `Ch4` structs exactly as in §Data Structures. Add `pub mod apu;` and `use crate::apu::Apu;` to `crates/gpuboy-core/src/lib.rs`. *(req 1)*

- [ ] 2. Implement `Apu::new(sample_rate: f32) -> Apu`. Initialize: `enabled = false`, `nr50 = 0x77`, `nr51 = 0xF3`, `fs_cycles = 0`, `fs_step = 0`, `sample_cycles = 0.0`, `sample_period = 4194304.0 / sample_rate`. All channel fields zero/false/disabled. Ch4: `lfsr = 0x7FFF`. *(req 2, 3)*

- [ ] 3. Implement `Apu::read(&self, addr: u16) -> u8` following §`Apu` register map. Route wave RAM reads (0xFF30–0xFF3F) to `self.ch3.wave_ram[(addr - 0xFF30) as usize]`. *(req 1, 9)*

- [ ] 4. Implement `Apu::write(&mut self, addr: u16, val: u8)`. When `!self.enabled && addr != 0xFF26`, return immediately. Route each address per §register map. For NRx4 trigger writes (bit 7 set): call the appropriate trigger handler. NR52 write: handle enable/disable transitions per §register map write behavior. Route wave RAM writes to `self.ch3.wave_ram`. *(req 1, 2, 5, 7)*

- [ ] 5. Implement private trigger handlers `trigger_ch1`, `trigger_ch2`, `trigger_ch3`, `trigger_ch4` on `Apu`. Follow the 8-step §Trigger Behavior exactly. *(req 5, 7)*

- [ ] 6. Implement `Apu::step(&mut self, t_cycles: u32, out: &mut Vec<f32>)`. Per T-cycle: decrement all four `freq_timer`s; on 0 reload and advance duty/wave/LFSR per §Frequency Timer Clocking. Accumulate `fs_cycles`; on ≥ 8192 subtract and call `tick_frame_sequencer`. Accumulate `sample_cycles`; on ≥ `sample_period` subtract and call `mix_sample`. When `!self.enabled`, skip channel updates and push `[0.0, 0.0]` at sample threshold. *(req 2, 3, 4, 6, 8)*

- [ ] 7. Implement `Apu::tick_frame_sequencer(&mut self)` following §Frame Sequencer Tick Schedule. Advance `fs_step = (fs_step + 1) % 8`. *(req 4, 6, 8)*

- [ ] 8. Implement `Apu::tick_sweep(&mut self)` following §Sweep Calculation exactly. *(req 6)*

- [ ] 9. Implement `Apu::mix_sample(&self, out: &mut Vec<f32>)` following §Sample Mixing Formula. *(req 3, 9)*

- [ ] 10. Implement sample helpers as private functions in `apu.rs`: `duty_sample(duty: u8, pos: u8) -> f32`, `wave_sample(wave_ram: &[u8; 16], pos: u8, output_level: u8) -> f32`, `noise_sample(lfsr: u16) -> f32`. *(req 3)*

- [ ] 11. Add `pub apu: Apu` to `Bus` in `bus.rs`. In `Bus::new` add `apu: Apu::new(44100.0)`. In `Bus::read` add `0xFF10..=0xFF3F => self.apu.read(addr)` before the catch-all. In `Bus::write` add `0xFF10..=0xFF3F => self.apu.write(addr, val)` before the catch-all. Add `pub fn step_apu(&mut self, t_cycles: u32, out: &mut Vec<f32>) { self.apu.step(t_cycles, out); }`. *(req 1)*

- [ ] 12. Add `Emulator::step_samples` to `lib.rs` following §`Emulator::step_samples` exactly. Keep `step_frame` unchanged. *(req 3)*

- [ ] 13. Add smoke test in `apu.rs` under `#[cfg(test)]`:
    ```rust
    #[test]
    fn apu_generates_samples() {
        let mut apu = Apu::new(44100.0);
        apu.enabled = true;
        apu.nr50 = 0x77;
        apu.nr51 = 0xFF;
        apu.write(0xFF11, 0x80); // duty=2, length=0
        apu.write(0xFF12, 0xF0); // env initial=15, add=false, period=0
        apu.write(0xFF13, 0x00);
        apu.write(0xFF14, 0x87); // trigger + freq high = 7
        let mut out = Vec::new();
        apu.step(10000, &mut out);
        assert!(out.len() >= 200, "expected at least 100 stereo pairs");
        assert!(out.iter().any(|&s| s > 0.0), "expected non-zero samples from CH1");
    }
    ```
    *(req 3)*

## Manual Testing

1. `cargo test -p gpuboy-core` — all tests pass including `apu_generates_samples`.
2. `cargo clippy -- -D warnings` — clean.
3. No browser testing required for this phase.

**Green light:** [ ]
