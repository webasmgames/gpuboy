# Phase 7: Joypad Input

## Overview

Wire up all three input paths — keyboard, gamepad, and touch — so games are actually playable. The HTML shell already has all 8 button elements. The emulator currently has no joypad register (0xFF00 falls through to `0xFF`). This phase implements the DMG P1 register in Rust, exposes a `set_buttons(u8)` call across the WASM boundary, and connects it to keyboard events, Gamepad API polling, and pointer events on the on-screen buttons.

## Requirements

1. WHEN the user holds W/A/S/D, THEN the corresponding D-pad direction is active in the emulator.
2. WHEN the user holds ArrowRight/ArrowLeft/ArrowDown/ArrowUp, THEN A/B/Start/Select is active respectively.
3. WHEN a gamepad is connected and a button or analog stick is pressed, THEN the corresponding GB button is active.
4. WHEN the user touches or clicks an on-screen D-pad or face button, THEN the corresponding GB button is active for the duration of the press.
5. WHEN 0xFF00 is read, THEN bits 7–6 are always 1, bits 5–4 reflect the written select lines, and bits 3–0 are active-low states for the selected row(s).
6. WHEN any selected button transitions from released to pressed, THEN IF bit 4 (joypad interrupt) is set.

## Acceptance Criteria

- [ ] Pressing W/A/S/D moves a character or navigates a menu in a real ROM
- [ ] Pressing ArrowRight fires A; ArrowLeft fires B; ArrowDown fires Start; ArrowUp fires Select
- [ ] Gamepad D-pad and face buttons (A=buttons[0], B=buttons[1], Select=buttons[8], Start=buttons[9]) work
- [ ] Tapping on-screen `#dpad-up/down/left/right`, `#btn-a`, `#btn-b`, `#btn-select`, `#btn-start` activates the correct button
- [ ] Arrow keys do not scroll the page while a ROM is running
- [ ] `bus.read(0xFF00)` returns `0xFF` with no buttons pressed and select=0x30 (existing test still passes)
- [ ] `bus.read(0xFF00)` returns correct active-low value when buttons are pressed and a row is selected
- [ ] No regressions: audio, play/pause, zoom, renderer toggle, hamburger menu all still work

## Design

### Architecture

Three new layers:
1. **`Joypad` struct in `bus.rs`** — owns the register state, produces the correct read value
2. **`Emulator::set_buttons(u8)` in `lib.rs`** — thin delegation to bus
3. **`set_buttons(u8)` WASM export in `gpuboy-wasm/src/lib.rs`** — thin delegation to emulator
4. **JS input handlers in `www/index.js`** — keyboard + gamepad + touch → call `set_buttons`
5. **CSS in `www/style.css`** — `touch-action: none` on button elements

### Data Structures

**Button bitmask (WASM/JS convention — 1 = pressed):**

```
bit 0 = A        bit 4 = Right
bit 1 = B        bit 5 = Left
bit 2 = Select   bit 6 = Up
bit 3 = Start    bit 7 = Down
```

**`Joypad` struct:**
```rust
struct Joypad {
    pressed: u8,  // bitmask above; 1 = pressed
    select: u8,   // bits 5-4 from last write to 0xFF00; default 0x30
}
```

`read_output()` → 4-bit active-low result:
- If P15 (bit 5 of `select`) is clear → action row: `!((Start<<3)|(Select<<2)|(B<<1)|A) & 0x0F`
- If P14 (bit 4 of `select`) is clear → direction row: `!((Down<<3)|(Up<<2)|(Left<<1)|Right) & 0x0F`
- Unselected row contributes `0x0F`; AND both row results

`read_full()` → `0xC0 | (select & 0x30) | read_output()`

Initial `select = 0x30` → read returns `0xFF` with no buttons pressed; existing `unmapped_reads_ff` test passes unchanged.

### Key Decisions

**Single `u8` bitmask over the WASM boundary** — simpler than 8 booleans; JS ORs keyboard + gamepad + touch bits before calling `set_buttons` once per event or RAF tick.

**Gamepad via `requestAnimationFrame`** — the Gamepad API is poll-only; RAF gives 60 fps polling without blocking the audio thread. Keyboard and touch remain event-driven.

**Standard gamepad mapping:** buttons[0]=A, buttons[1]=B, buttons[8]=Select, buttons[9]=Start, buttons[12–15]=D-pad; axes[0]/axes[1] as fallback with ±0.5 threshold.

**`touch-action: none` on button elements** — prevents browsers from claiming the touch for scroll, enabling responsive pointer events on mobile.

**Keyboard guard** — skip key events when `document.activeElement` is an `<input>` (covers the hidden file picker).

## Tasks

- [x] 1. Add `Joypad` struct to `crates/gpuboy-core/src/bus.rs` with `pressed: u8` and `select: u8 = 0x30`. Implement `read_output() -> u8` and `read_full() -> u8`. *(req 5)*
- [x] 2. Add `joypad: Joypad` field to `Bus` struct and `Bus::new()`. Handle `0xFF00` read (`self.joypad.read_full()`) and write (`self.joypad.select = val & 0x30`) in `Bus::read`/`Bus::write`. *(req 5)*
- [x] 3. Add `Bus::set_joypad_buttons(&mut self, pressed: u8)`: store new `pressed`, compare old vs new `read_output()`, fire `self.interrupt_flags |= 1 << 4` on any 1→0 transition. *(req 6)*
- [x] 4. Add `Emulator::set_buttons(&mut self, pressed: u8)` in `crates/gpuboy-core/src/lib.rs` delegating to `self.bus.set_joypad_buttons(pressed)`. *(req 5, 6)*
- [x] 5. Add `#[wasm_bindgen] pub fn set_buttons(pressed: u8)` to `crates/gpuboy-wasm/src/lib.rs` delegating to `EMULATOR`. *(req 1–4)*
- [x] 6. Add unit tests to `bus.rs`: direction-row read, action-row read, interrupt fires on press, interrupt does not fire on release. Confirm existing `unmapped_reads_ff` test still passes. *(req 5, 6)*
- [x] 7. In `www/index.js`: add keyboard handler (`KEY_CODE_MAP` for WASD + arrows), `keydown`/`keyup` listeners, `joyBits` bitmask, `preventDefault()` on mapped keys, call `set_buttons`. *(req 1, 2)*
- [x] 8. In `www/index.js`: add gamepad RAF polling loop (`pollGamepad` → `navigator.getGamepads()` → map buttons + axes → OR with `joyBits` + `touchBits` → `set_buttons`). Start loop after WASM init. *(req 3)*
- [x] 9. In `www/index.js`: attach `pointerdown`/`pointerup`/`pointercancel` to all 8 on-screen button elements (`#dpad-*`, `#btn-a`, `#btn-b`, `#btn-select`, `#btn-start`), maintaining `touchBits`. *(req 4)*
- [x] 10. In `www/style.css`: add `touch-action: none` to D-pad buttons and face buttons. *(req 4)*

## Manual Testing

1. Run `python -m http.server 8000` and open `http://localhost:8000/www/`.
2. Load a sample ROM from the hamburger menu (e.g. Fairy Lake).
3. Press W/A/S/D — confirm D-pad movement in the game.
4. Press ArrowRight (A), ArrowLeft (B), ArrowDown (Start), ArrowUp (Select) — confirm actions.
5. Connect a gamepad; press D-pad and face buttons — confirm correct mapping.
6. On mobile or with Chrome DevTools touch simulation: tap on-screen D-pad and A/B buttons — confirm input.
7. Verify arrow keys do not scroll the page while the game is running.
8. Verify play/pause, audio mute, zoom, and renderer toggle still work.

**Green light:** [x]
