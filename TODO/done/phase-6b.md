# Phase 6b: Web UI — Sample ROMs

## Overview

Adds a sample ROM catalog to the hamburger menu so users can try the emulator without hunting down ROM files. Clicking a ROM name fetches it from a GitHub raw URL (`raw.githubusercontent.com` serves `Access-Control-Allow-Origin: *`), calls `load_rom()`, and starts audio — the same code path as the file picker. No ROM files are committed to the repo. No Rust/WASM changes needed.

## Requirements

1. WHEN the hamburger menu is open, THEN a horizontal separator and a "Sample ROMs" section appear below the zoom buttons, listing at least three ROM names.

2. WHEN the user clicks a sample ROM name and no ROM is currently loading, THEN the button label changes to "Loading…" and the browser begins fetching the ROM from its GitHub raw URL.

3. WHEN the fetch completes successfully, THEN `load_rom()` is called with the ROM bytes, `start_audio()` is called with the render callback, the menu closes, and the button label returns to the ROM name.

4. WHEN the fetch fails (network error or non-200 response), THEN the `#error` div displays "Failed to load [name]: [reason]" and the button label returns to the ROM name.

5. WHEN a ROM fetch is in progress (a button shows "Loading…"), THEN clicking any sample ROM button has no effect.

## Acceptance Criteria

- [ ] Hamburger menu shows separator + "SAMPLE ROMS" label + at least three ROM name buttons below the zoom group.
- [ ] Clicking a ROM shows "Loading…" on that button.
- [ ] After load: menu closes, emulation starts (screen updates, audio plays).
- [ ] While loading, clicking any ROM button is a no-op.
- [ ] Network failure shows error in `#error`; button label restores to ROM name.
- [ ] No regressions: file picker, audio toggle, play/pause, zoom, renderer toggle all still work.

## Design

### Architecture

Only `www/index.js`, `www/index.html`, and `www/style.css` change. No changes to any crate.

```
www/
  index.html   — add <hr>, label div, and <div id="menu-roms"> inside #gb-menu
  index.js     — add SAMPLE_ROMS catalog; extract loadRomBytes(); generate list + handlers
  style.css    — add .menu-sep, .menu-roms-label, #menu-roms button styles
```

### Data Structures

```js
const SAMPLE_ROMS = [
  { name: 'cpu_instrs (Blargg)',  url: 'https://github.com/retrio/gb-test-roms/raw/master/cpu_instrs/cpu_instrs.gb' },
  { name: 'mem_timing (Blargg)',  url: 'https://github.com/retrio/gb-test-roms/raw/master/mem_timing/mem_timing.gb' },
  { name: '2048 (Sanqui)',        url: 'https://github.com/Sanqui/2048-gb/raw/master/2048.gb' },
];
```

A module-level `let romLoading = false` flag prevents concurrent fetches.

### Key Decisions

**Fetch-on-demand, no committed ROM files.** `raw.githubusercontent.com` allows cross-origin fetch. Legal (open-source / freely distributed test ROMs). No repo bloat.

**Dynamic menu generation from a JS catalog.** Loop over `SAMPLE_ROMS` at page-load time, create `<button>` elements, append into `#menu-roms`. Avoids duplicating catalog data between HTML and JS; adding a ROM only requires editing the array.

**Shared `loadRomBytes(data)` helper.** Extract the `load_rom()` + `start_audio()` call from the file-picker `onload` handler into a standalone function. Both code paths call it — single source of truth.

**Reuse `#error` div for fetch errors.** Already present and styled from Phase 3b. Cleared on the next successful load.

**`romLoading` flag instead of disabling all buttons.** Simpler than managing disabled state across dynamically created buttons; early-return is sufficient.

### HTML addition inside `#gb-menu` (after `.menu-zoom-group`)

```html
<hr class="menu-sep">
<div class="menu-roms-label">Sample ROMs</div>
<div id="menu-roms"></div>
```

### CSS additions

```css
.menu-sep { border: none; border-top: 1px solid #555; margin: 4px 0; }
.menu-roms-label { font-size: 0.7rem; color: var(--gb-text-dim); padding: 2px 8px; text-transform: uppercase; letter-spacing: 0.05em; }
#menu-roms button { width: 100%; text-align: left; }
```

If the full menu overflows viewport height, add `max-height` and `overflow-y: auto` to `.gb-menu`.

## Tasks

- [x] 1. Verify the three GitHub raw URLs in the catalog actually resolve to valid `.gb` files (fetch in browser or curl). If any URL is broken, find a working mirror and update the catalog before proceeding. *(req 2, 3)*

- [x] 2. In `www/index.js`, extract the `load_rom(data)` + `start_audio(callback)` block from inside the `rom-picker` `onload` handler into a standalone `function loadRomBytes(data)`. Update the file-picker handler to call `loadRomBytes()`. *(housekeeping; req 3)*

- [x] 3. Add `const SAMPLE_ROMS = [{name, url}, …]` (using verified URLs from task 1) and `let romLoading = false` to `www/index.js` at module scope, before `main()`. *(req 1, 2, 5)*

- [x] 4. Inside `main()` in `www/index.js`, after the zoom handlers, loop over `SAMPLE_ROMS` and for each entry: create a `<button>`, set its `textContent` to `entry.name`, attach a click handler that (a) early-returns if `romLoading`; (b) sets `romLoading = true` and button text to `'Loading…'`; (c) fetches the URL and checks `response.ok`; (d) on success, calls `loadRomBytes()`, closes the menu (`gbMenu.classList.add('hidden')`), restores button text, and clears `romLoading`; (e) on failure, sets `#error` text to `'Failed to load [name]: [reason]'`, restores button text, and clears `romLoading`. Append buttons into `document.getElementById('menu-roms')`. *(req 2, 3, 4, 5)*

- [x] 5. In `www/index.html`, inside `#gb-menu` after the `.menu-zoom-group` div, add `<hr class="menu-sep">`, `<div class="menu-roms-label">Sample ROMs</div>`, and `<div id="menu-roms"></div>`. *(req 1)*

- [x] 6. In `www/style.css`, add styles for `.menu-sep`, `.menu-roms-label`, and `#menu-roms button`. If needed, add `max-height` + `overflow-y: auto` to `.gb-menu` so the extended menu doesn't overflow a short viewport. *(req 1)*

## Manual Testing

1. Build WASM: `wasm-pack build crates/gpuboy-wasm --target web`
2. Serve: `python -m http.server 8000`. Open `http://localhost:8000/www/` in Chrome.
3. Open hamburger. Confirm separator + "SAMPLE ROMS" label + three ROM buttons appear below the zoom group.
4. Click `cpu_instrs (Blargg)`. Confirm button shows "Loading…". After load, menu closes and emulation starts (screen updates; Blargg ROM prints CPU test results via serial).
5. Click `2048 (Sanqui)` from the menu. Confirm 2048 title screen appears and game responds to input.
6. While a ROM is loading (button shows "Loading…"), click another ROM button. Confirm nothing happens (no second fetch).
7. In DevTools → Network, set throttling to Offline. Click a ROM. Confirm the `#error` area shows "Failed to load …" and the button label restores.
8. Re-enable network. Load a ROM via the folder icon (file picker). Confirm it still works as before.
9. Confirm audio toggle, play/pause, zoom (2×/3×/4×), and renderer toggle still work after loading a sample ROM.
10. Run `cargo clippy` and `cargo fmt -- --check`. Confirm no warnings or errors.

**Green light:** [x]
