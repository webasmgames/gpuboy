# Phase 6: Web UI

## Overview

Replaces the bare-bones file input + canvas with a proper Game Boy shell UI. The page renders a Classic DMG-styled shell (charcoal body, cream screen bezel, purple accent buttons, speaker grill) that holds the existing canvas. A toolbar handles ROM loading, audio muting, and play/pause. A hamburger menu provides the renderer toggle and zoom control. D-pad, A/B, Select, and Start controls are laid out in the correct positions as visual stubs — Phase 7 wires their events. All styling lives in a new `www/style.css`; the WASM boundary gains two small new exports (`set_volume`, `set_paused`) so the toolbar can control audio and emulation state without JS needing direct access to the `AudioContext`.

**Prerequisites:** Phase 5b must be complete before starting. Phase 6b (sample ROM dropdown) is a follow-on phase.

## Requirements

1. WHEN the page loads, THEN the interface displays a Game Boy-styled shell: charcoal gray body, cream screen bezel, purple A/B buttons, a dark cross D-pad, and a diagonal-line speaker grill in the bottom-right area of the shell.

2. WHEN the viewport width is ≥ 640px, THEN the D-pad, screen bezel, and A/B buttons are arranged in a single horizontal row, with Select/Start centered below the screen.

3. WHEN the viewport width is < 640px, THEN the layout stacks vertically in this order: toolbar, D-pad, screen, A/B buttons, Select/Start.

4. WHEN the user clicks the folder icon in the toolbar, THEN the OS file picker opens filtered to `.gb` and `.gbc` files.

5. WHEN the user selects a ROM file via the folder icon, THEN the ROM loads and emulation starts exactly as it did in Phase 5b.

6. WHEN the user clicks the audio toggle icon and audio is currently unmuted, THEN audio output is silenced (gain = 0) and the icon changes to a muted state; emulation continues running.

7. WHEN the user clicks the audio toggle icon and audio is currently muted, THEN audio output is restored (gain = 1) and the icon returns to the unmuted state.

8. WHEN the user clicks the play/pause icon and emulation is running, THEN the `onaudioprocess` callback outputs silence without advancing the emulator, the canvas freezes on the last rendered frame, and the icon changes to a paused state.

9. WHEN the user clicks the play/pause icon and emulation is paused, THEN the emulator resumes from its frozen state and the icon returns to the playing state.

10. WHEN the user clicks the hamburger icon, THEN a dropdown menu appears containing a renderer toggle item and three zoom options (2×, 3×, 4×); clicking anywhere outside the menu closes it.

11. WHEN the user selects a zoom level (2×, 3×, or 4×) from the hamburger menu, THEN the canvas CSS size changes to 320×288, 480×432, or 640×576 respectively, with `image-rendering: pixelated` preserved.

12. WHEN the user clicks the renderer toggle item in the hamburger menu, THEN the active renderer switches between WebGPU (`#screen`) and 2D canvas (`#screen-2d`) exactly as the old toggle button did.

13. WHEN the page renders, THEN the D-pad (`#dpad` with children `#dpad-up`, `#dpad-down`, `#dpad-left`, `#dpad-right`), A/B buttons (`#btn-a`, `#btn-b`), and Select/Start buttons (`#btn-select`, `#btn-start`) are present in the DOM as inert stubs (no event listeners — Phase 7 adds those).

## Acceptance Criteria

- [ ] Page loads showing a charcoal Game Boy shell with cream bezel and purple A/B buttons. No console errors.
- [ ] Folder icon opens the OS file picker. Selecting a `.gb` ROM starts emulation and audio as before.
- [ ] Audio toggle mutes audio (canvas keeps updating) and icon reflects muted state; toggling again restores audio.
- [ ] Play/pause button freezes canvas and silences audio (emulator state preserved); toggling again resumes from frozen state.
- [ ] Hamburger opens a dropdown with renderer toggle + zoom options; clicking outside closes it.
- [ ] Zoom 2× sets canvas CSS to 320×288, 3× to 480×432, 4× to 640×576 with pixelated rendering.
- [ ] Renderer toggle in hamburger switches between WebGPU and 2D canvas paths.
- [ ] Speaker grill (diagonal stripe decoration) is visible in the shell's lower-right area.
- [ ] D-pad cross shape rendered; `#dpad-up`, `#dpad-down`, `#dpad-left`, `#dpad-right` exist in DOM.
- [ ] `#btn-a`, `#btn-b`, `#btn-select`, `#btn-start` exist in DOM.
- [ ] At viewport < 640px, layout stacks vertically (toolbar → D-pad → screen → A/B → Select/Start).
- [ ] No regressions: ROM loading, audio playback, WebGPU and 2D canvas rendering all work.

## Design

### Architecture

```
www/
  index.html    — rewritten: GB shell structure, inline SVG icons, links style.css
  index.js      — updated: set_volume/set_paused imports; toolbar handlers; zoom logic;
                  hamburger menu toggle; renderer toggle moved from button to menu
  style.css     — new: all styling; CSS custom properties for DMG palette
crates/
  gpuboy-wasm/
    Cargo.toml  — add GainNode, AudioParam to web-sys features
    src/lib.rs  — add GAIN_NODE thread-local + set_volume export;
                  add PAUSED thread-local + set_paused export;
                  modify start_audio to wire GainNode into the audio graph
```

No changes to `gpuboy-core` or `gpuboy-render`.

### Data Structures

**New thread-locals in `gpuboy-wasm/src/lib.rs`:**

```rust
thread_local! {
    static GAIN_NODE: RefCell<Option<web_sys::GainNode>> = const { RefCell::new(None) };
    static PAUSED: Cell<bool> = const { Cell::new(false) };
}
```

**Audio graph (after Phase 6):**
```
ScriptProcessorNode → GainNode → AudioContext.destination
```

Previously `ScriptProcessorNode` connected directly to `destination`. The GainNode is inserted in `start_audio` for muting control.

### Key Decisions

**Inline SVG icons, no icon font or CDN.** The project uses no bundler and no external dependencies. Four simple SVG paths (≈ 5 lines each) for hamburger, folder, speaker, and play/pause keep things self-contained and work offline.

**`set_volume` / `set_paused` as WASM exports rather than returning the `AudioContext` to JS.** The `AudioContext` and audio graph already live in Rust thread-locals from Phase 5b. Exporting `set_volume(f32)` and `set_paused(bool)` is a two-function surface that JS can call without knowing anything about the audio graph internals. The alternative — changing `start_audio` to return the `AudioContext` as a `JsValue` — would require breaking the Phase 5b signature.

**`set_paused` checks a flag in the `onaudioprocess` closure.** When `PAUSED` is `true`, the closure outputs silence without calling `step_samples`. This freezes the emulator without modifying the audio graph topology (no connect/disconnect). The last rendered frame stays on canvas because `on_frame` is not called.

**Zoom via CSS only, no Rust changes.** Zoom changes the canvas element's CSS `width` and `height` — the intrinsic size stays 160×144. `image-rendering: pixelated` is already set. No WASM changes needed.

**Speaker grill via CSS `repeating-linear-gradient`.** A series of diagonal stripes at 45° using a dark body color and a slightly lighter color. Purely decorative, no extra DOM elements needed beyond a single `<div class="gb-speaker">`.

**D-pad implemented as a CSS grid.** Three-column, three-row grid with buttons at the four cardinal positions and an inert center cell. IDs (`#dpad-up` etc.) allow Phase 7 to attach `touchstart`/`touchend`/`pointerdown`/`pointerup` listeners without touching Phase 6 markup.

**Default zoom is 3× (480×432).** Matches the current behavior.

### CSS Custom Properties (DMG Palette)

```css
:root {
    --gb-body:        #3a3a3a;
    --gb-body-dark:   #2a2a2a;
    --gb-bezel:       #c8c4b0;
    --gb-screen-bg:   #1a1a1a;
    --gb-accent:      #8b3a8b;
    --gb-accent-dark: #6b2a6b;
    --gb-btn-ab:      #9b2d8b;
    --gb-btn-dpad:    #2a2a2a;
    --gb-btn-sys:     #666;
    --gb-text:        #e0e0e0;
    --gb-text-dim:    #999;
}
```

### HTML Structure

```html
<div class="gb-shell">
  <div class="gb-toolbar">
    <button id="btn-menu">   <!-- hamburger SVG -->  </button>
    <button id="btn-folder"> <!-- folder SVG -->     </button>
    <button id="btn-audio">  <!-- speaker SVG -->    </button>
    <button id="btn-playpause"> <!-- play SVG -->    </button>
    <div id="gb-menu" class="gb-menu hidden">
      <button id="menu-renderer">Switch to 2D canvas</button>
      <div class="menu-zoom-group">
        <button class="zoom-btn" data-zoom="2">2×</button>
        <button class="zoom-btn active" data-zoom="3">3×</button>
        <button class="zoom-btn" data-zoom="4">4×</button>
      </div>
    </div>
  </div>

  <input type="file" id="rom-picker" accept=".gb,.gbc" style="display:none">

  <div class="gb-body">
    <div class="gb-left">
      <div id="dpad">
        <button id="dpad-up"></button>
        <button id="dpad-left"></button>
        <div class="dpad-center"></div>
        <button id="dpad-right"></button>
        <button id="dpad-down"></button>
      </div>
    </div>

    <div class="gb-center">
      <div class="gb-bezel">
        <canvas id="screen" width="160" height="144"></canvas>
        <canvas id="screen-2d" width="160" height="144" style="display:none"></canvas>
        <div id="error"></div>
      </div>
      <div class="gb-select-start">
        <button id="btn-select">SELECT</button>
        <button id="btn-start">START</button>
      </div>
    </div>

    <div class="gb-right">
      <div class="gb-ab">
        <button id="btn-b">B</button>
        <button id="btn-a">A</button>
      </div>
    </div>
  </div>

  <div class="gb-speaker"></div>
</div>
```

### JS Changes (`www/index.js`)

Updated WASM import — add `set_volume`, `set_paused`; drop `get_framebuffer` (unused since Phase 5b):
```js
import init, { run, load_rom, init_renderer, render_frame_wgpu, start_audio,
                set_volume, set_paused }
    from "../pkg/gpuboy_wasm.js";
```

New state variables:
```js
let audioMuted = false;
let emulatorPaused = false;
```

Event handlers:
- `#btn-folder` click → `document.getElementById('rom-picker').click()`
- `#btn-audio` click → toggles `audioMuted`; calls `set_volume(audioMuted ? 0.0 : 1.0)`; swaps SVG icon
- `#btn-playpause` click → toggles `emulatorPaused`; calls `set_paused(emulatorPaused)`; swaps SVG icon
- `#btn-menu` click → toggles `hidden` class on `#gb-menu`
- `document` click (capture) → closes `#gb-menu` when click target is outside menu/button
- `#menu-renderer` click → existing renderer toggle logic (replaces the old `#renderer-toggle` button)
- `.zoom-btn` click → sets canvas CSS size based on `data-zoom`; updates `active` class
- `#rom-picker` change → existing ROM load + `start_audio(...)` call (unchanged)

Remove: `#renderer-toggle` button handling. The button no longer exists in the HTML.

### Rust Changes (`crates/gpuboy-wasm/src/lib.rs`)

**`start_audio` modification** — insert GainNode:
```rust
let gain_node = ctx.create_gain()?;
gain_node.gain().set_value(1.0);
script_node.connect_with_audio_node(&gain_node)?;
gain_node.connect_with_audio_node(&ctx.destination())?;
// (previously: script_node.connect_with_audio_node(&ctx.destination())?)
GAIN_NODE.with(|g| *g.borrow_mut() = Some(gain_node));
```

**New exports:**
```rust
#[wasm_bindgen]
pub fn set_volume(vol: f32) {
    GAIN_NODE.with(|g| {
        if let Some(node) = g.borrow().as_ref() {
            node.gain().set_value(vol);
        }
    });
}

#[wasm_bindgen]
pub fn set_paused(paused: bool) {
    PAUSED.with(|p| p.set(paused));
}
```

**`onaudioprocess` closure modification** — check `PAUSED` at top:
```rust
let closure = Closure::wrap(Box::new(move |event: web_sys::AudioProcessingEvent| {
    let paused = PAUSED.with(|p| p.get());
    let output = match event.output_buffer() { Ok(buf) => buf, Err(_) => return };
    if paused {
        if let (Ok(l), Ok(r)) = (output.get_channel_data(0), output.get_channel_data(1)) {
            l.copy_from(&vec![0.0f32; n]);
            r.copy_from(&vec![0.0f32; n]);
        }
        return;
    }
    // ... existing step_samples / render logic unchanged ...
```

## Tasks

- [x] 1. Add `GainNode` and `AudioParam` to the `web-sys` features list in `crates/gpuboy-wasm/Cargo.toml`. *(housekeeping)*

- [x] 2. Add `GAIN_NODE: RefCell<Option<web_sys::GainNode>>` and `PAUSED: Cell<bool>` thread-locals to `crates/gpuboy-wasm/src/lib.rs`. Add `use std::cell::Cell;` if not already imported. *(req 6, 7, 8, 9)*

- [x] 3. Modify `start_audio` in `crates/gpuboy-wasm/src/lib.rs`: create a `GainNode`, set its initial gain to 1.0, connect `script_node → gain_node → destination` (replacing the direct `script_node → destination` connection), store the gain node in `GAIN_NODE`. *(req 6, 7)*

- [x] 4. Add the `PAUSED` check at the top of the `onaudioprocess` closure in `start_audio`: if `PAUSED` is true, write silence to both channels and `return` without calling `step_samples` or `on_frame`. *(req 8, 9)*

- [x] 5. Add `set_volume(vol: f32)` WASM export that sets `GAIN_NODE`'s gain value. Add `set_paused(paused: bool)` WASM export that sets the `PAUSED` cell. *(req 6, 7, 8, 9)*

- [x] 6. Create `www/style.css` with: CSS custom properties for the DMG color palette; `body` reset (background, centering); `.gb-shell` (charcoal, rounded corners, padding, relative position, max-width ~700px); `.gb-toolbar` (flex row, icons aligned left, position relative); `.gb-menu` (absolute dropdown, hidden class); `.gb-body` (flex row, align-items center, gap); `.gb-left` / `.gb-right` (flex column, center-aligned); `#dpad` (CSS grid 3×3, `--gb-btn-dpad` buttons, cross shape via grid-area); `.gb-center` (flex column, align center); `.gb-bezel` (cream background, padding, rounded corners, inline-flex); `#screen`, `#screen-2d` (default CSS width 480px / height 432px, `image-rendering: pixelated`); `.gb-select-start` (flex row, gap, margin-top); `#btn-select`, `#btn-start` (small rounded pill buttons, `--gb-btn-sys`); `.gb-ab` (flex column, gap, align-end); `#btn-a`, `#btn-b` (round circle buttons, `--gb-btn-ab`, purple); `.gb-speaker` (fixed size div, `repeating-linear-gradient` diagonal stripes at 45°, align bottom-right within shell); toolbar button styling (transparent background, `--gb-text` fill for SVGs, hover state); `.zoom-btn.active` (highlighted state). *(req 1, 2, 11, 12, 13)*

- [x] 7. Rewrite `www/index.html` with the GB shell structure from §HTML Structure. Include `<link rel="stylesheet" href="style.css">`. Remove the old `<input type="file">` from its visible position and the old `<button id="renderer-toggle">`. Move the file input inside the shell as `style="display:none"`. Use inline SVG for hamburger, folder, speaker/mute, and play/pause icons (two SVGs per toggle button — one visible, one `display:none`). *(req 1, 2, 3, 4, 13)*

- [x] 8. Update `www/index.js`: add `set_volume` and `set_paused` to the WASM import line. Add `audioMuted = false` and `emulatorPaused = false` state variables. Add event handlers for `#btn-folder` (triggers `#rom-picker` click), `#btn-audio` (toggles mute, calls `set_volume`, swaps icon), `#btn-playpause` (toggles pause, calls `set_paused`, swaps icon), `#btn-menu` (toggles `#gb-menu` hidden), document click listener to close menu when clicking outside. Move renderer toggle logic from the removed `#renderer-toggle` button to the `#menu-renderer` menu item. Add `.zoom-btn` click handler that reads `data-zoom`, updates `#screen` / `#screen-2d` CSS size, and updates `.active` class. Remove all references to the old `#renderer-toggle` button. *(req 4, 5, 6, 7, 8, 9, 10, 11, 12)*

- [x] 9. Add responsive breakpoint in `www/style.css`: at `max-width: 639px`, change `.gb-body` to `flex-direction: column` and reorder/center the left/center/right sections stacked vertically. *(req 3)*

- [x] 10. Verify `www/index.js` removes the `get_framebuffer` import if it was still present (it was unused since Phase 5b). *(housekeeping)*

## Manual Testing

1. Build WASM: `wasm-pack build crates/gpuboy-wasm --target web`
2. Serve: `python -m http.server 8000`. Open `http://localhost:8000/www/` in Chrome.
3. Confirm page loads with the Game Boy shell: charcoal body, cream screen bezel, purple A/B buttons, D-pad cross. No console errors.
4. Click the folder icon. Confirm the OS file picker opens filtered to `.gb`/`.gbc`. Load `tetris.gb`. Confirm emulation starts: Tetris title screen visible, audio playing.
5. Click the audio toggle. Confirm audio goes silent and the icon changes to muted state. Canvas should still be updating. Click again — confirm audio resumes.
6. Click play/pause. Confirm the canvas freezes and audio goes silent. Click again — confirm emulation resumes from where it left off.
7. Click the hamburger. Confirm a dropdown appears with a renderer toggle item and 2×/3×/4× zoom buttons. Click somewhere outside — confirm the dropdown closes.
8. In the hamburger menu, click 4×. Confirm the canvas CSS size expands to 640×576 with no blur (pixelated). Click 2× — confirm it shrinks to 320×288. Click 3× to restore default.
9. If WebGPU is available: click the renderer toggle in the hamburger. Confirm the renderer switches to 2D canvas. Click again to switch back.
10. Narrow the browser window below 640px. Confirm the layout stacks: D-pad above screen, A/B below screen, all elements visible.
11. Confirm `#dpad-up`, `#dpad-down`, `#dpad-left`, `#dpad-right`, `#btn-a`, `#btn-b`, `#btn-select`, `#btn-start` are present in the DOM (inspect in DevTools). Clicking them should do nothing (no Phase 7 events yet).
12. Run `cargo clippy` and `cargo fmt -- --check`. Confirm no warnings or errors.

**Green light:** [x]
