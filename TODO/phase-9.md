# Phase 9: Save Data

## Overview

Persist battery-backed SRAM to the browser's IndexedDB so game progress survives page reloads. The save key is the 16-bit global checksum stored in the ROM header at 0x014E–0x014F. Players can also export raw `.sav` files for backup or import them from another source. Save-related UI is only shown when the loaded ROM has SRAM.

## Requirements

1. WHEN a ROM with SRAM is loaded, THEN the app reads the ROM global checksum, looks up IndexedDB for a saved snapshot under that key, and restores it via `set_sram` if found.
2. WHEN a ROM with SRAM is running, THEN the SRAM is written to IndexedDB every 30 seconds automatically.
3. WHEN the user clicks "Save game" in the hamburger menu, THEN the current SRAM is persisted to IndexedDB immediately and the button label shows "Saved!" for 1 second before reverting.
4. WHEN a new ROM is loaded while a ROM with SRAM is already running, THEN the outgoing SRAM is saved to IndexedDB before the emulator is replaced.
5. WHEN the user clicks "Export .sav", THEN the current SRAM bytes are downloaded as `<rom-title>.sav`.
6. WHEN the user clicks "Import save" and selects a `.sav` file, THEN the file bytes are passed to `set_sram` and immediately persisted to IndexedDB under the current checksum key.
7. WHEN a ROM with no SRAM is loaded, THEN save-related menu items and their separator are hidden.
8. WHEN a ROM with SRAM is loaded, THEN "Save game", "Export .sav", and "Import save" are visible in the hamburger menu.

## Acceptance Criteria

- [ ] Load a battery-backed ROM (e.g. Pokémon), play briefly, wait 30 s. Reload the page. Progress is restored.
- [ ] "Save game" immediately persists (reload without waiting 30 s confirms it).
- [ ] "Export .sav" downloads a file named `<title>.sav` whose byte length matches the expected SRAM size.
- [ ] "Import save" replaces the running SRAM; subsequent reload restores the imported state.
- [ ] Loading a ROM with no SRAM (e.g. Tetris — no SRAM in header) hides all save menu items.
- [ ] Switching from a save-bearing ROM to another loads the second ROM's save (not the first's).
- [ ] `./preflight.sh` passes (no regressions in Rust layer).

## Design

### Architecture

Three layers each get small additions:

**gpuboy-core** — `Cartridge` gains four methods (`get_sram`, `set_sram`, `sram_len`, `rom_checksum`); `Emulator` delegates them plus `rom_title`.

**gpuboy-wasm** — thin `#[wasm_bindgen]` exports for each new core method.

**www/index.js** — IndexedDB helpers, async `loadRomBytes`, autosave interval, and three menu-item handlers. No new dependencies.

### IndexedDB schema

- Database name: `"gpuboy-saves"`, version 1
- Object store: `"sram"`, out-of-line keys
- Key format: 4-char lowercase hex string of the 16-bit global checksum, e.g. `"a3f2"`
- Value: `Uint8Array` (raw SRAM bytes; structured clone preserves the typed array)

### Key Decisions

**Global checksum as key.** The two bytes at 0x014E–0x014F are a precomputed 16-bit sum of the ROM. Collision risk is low for retail ROMs; reading them is zero-cost (no iteration over the ROM). ROMs with no SRAM skip all IDB operations regardless of the key.

**`set_sram` is length-gated.** If the imported file length differs from the emulator's allocated SRAM, the call is a silent no-op. This prevents corrupting the emulator state when the user imports a save for the wrong game.

**No save on `beforeunload`.** `beforeunload` handlers are unreliable on mobile and may be blocked. The 30-second autosave covers crashes; the manual "Save game" button covers intentional exits.

**Save before ROM switch.** When `loadRomBytes` is called with a ROM already running, the outgoing SRAM is written to IDB before `load_rom` is called. This prevents data loss when the user opens a second game.

**Save UI hidden until SRAM confirmed.** `sram_len()` is called after `load_rom` returns. If zero, no IDB operations happen and the menu items stay hidden.

## Tasks

- [ ] 1. In `crates/gpuboy-core/src/cartridge.rs`, add four `pub` methods to `impl Cartridge`:

  ```rust
  pub fn get_sram(&self) -> Vec<u8> { self.sram.clone() }

  pub fn set_sram(&mut self, data: Vec<u8>) {
      if data.len() == self.sram.len() { self.sram = data; }
  }

  pub fn sram_len(&self) -> usize { self.sram.len() }

  pub fn rom_checksum(&self) -> u16 {
      let hi = self.rom.get(0x014E).copied().unwrap_or(0) as u16;
      let lo = self.rom.get(0x014F).copied().unwrap_or(0) as u16;
      (hi << 8) | lo
  }
  ```
  *(req 1–6)*

- [ ] 2. In `crates/gpuboy-core/src/lib.rs`, add delegate methods to `impl Emulator`:

  ```rust
  pub fn get_sram(&self) -> Vec<u8>        { self.bus.cartridge.get_sram() }
  pub fn set_sram(&mut self, d: Vec<u8>)   { self.bus.cartridge.set_sram(d) }
  pub fn sram_len(&self) -> usize           { self.bus.cartridge.sram_len() }
  pub fn rom_checksum(&self) -> u16         { self.bus.cartridge.rom_checksum() }
  pub fn rom_title(&self) -> &str           { &self.bus.cartridge.header.title }
  ```
  *(req 1–6)*

- [ ] 3. In `crates/gpuboy-wasm/src/lib.rs`, add `#[wasm_bindgen]` exports for all five methods. Pattern matches the existing `set_buttons` / `get_framebuffer` style. `get_sram` returns `Vec<u8>` (becomes `Uint8Array` in JS); `set_sram` takes `Vec<u8>`; `sram_len` returns `usize`; `rom_checksum` returns `u16`; `rom_title` returns `String`. Each accesses `EMULATOR` via `.with(|e| ...)` and returns a default (`Vec::new()`, 0, empty string) when no emulator is loaded. *(req 1–6)*

- [ ] 4. In `www/index.html`:
  - Add alongside `#rom-picker`: `<input type="file" id="sav-picker" accept=".sav" style="display:none">`. *(req 6)*
  - In `#gb-menu`, after `<div id="menu-roms"></div>`, add:
    ```html
    <hr class="menu-sep" id="menu-save-sep" style="display:none">
    <button id="menu-save" style="display:none">Save game</button>
    <button id="menu-export-sav" style="display:none">Export .sav</button>
    <button id="menu-import-sav" style="display:none">Import save</button>
    ```
  *(req 7, 8)*

- [ ] 5. In `www/index.js`, update the wasm import line to include `get_sram, set_sram, sram_len, rom_checksum, rom_title`. *(req 1–6)*

- [ ] 6. In `www/index.js`, add module-level state and IDB helpers after the `SAMPLE_ROMS` block:

  ```js
  let currentChecksum = null;
  let saveTick = null;

  function openDb() {
      return new Promise((resolve, reject) => {
          const req = indexedDB.open('gpuboy-saves', 1);
          req.onupgradeneeded = () => req.result.createObjectStore('sram');
          req.onsuccess = () => resolve(req.result);
          req.onerror = () => reject(req.error);
      });
  }
  async function loadSramIdb(key) {
      const db = await openDb();
      return new Promise((resolve, reject) => {
          const req = db.transaction('sram', 'readonly').objectStore('sram').get(key);
          req.onsuccess = () => resolve(req.result ?? null);
          req.onerror = () => reject(req.error);
      });
  }
  async function saveSramIdb(key, data) {
      const db = await openDb();
      return new Promise((resolve, reject) => {
          const tx = db.transaction('sram', 'readwrite');
          tx.objectStore('sram').put(data, key);
          tx.oncomplete = resolve;
          tx.onerror = () => reject(tx.error);
      });
  }
  ```
  *(req 1–4)*

- [ ] 7. In `www/index.js`, refactor `loadRomBytes` to `async`:

  ```js
  async function loadRomBytes(data) {
      // Persist outgoing SRAM before replacing emulator
      if (currentChecksum !== null && sram_len() > 0) {
          await saveSramIdb(currentChecksum, get_sram());
      }
      if (saveTick !== null) { clearInterval(saveTick); saveTick = null; }

      const errEl = document.getElementById('error');
      if (errEl) errEl.style.display = 'none';
      load_rom(data);

      const len = sram_len();
      if (len > 0) {
          const cs = rom_checksum().toString(16).padStart(4, '0');
          currentChecksum = cs;
          const saved = await loadSramIdb(cs);
          if (saved) set_sram(saved);
          saveTick = setInterval(() => saveSramIdb(cs, get_sram()), 30_000);
          updateSaveUI(true);
      } else {
          currentChecksum = null;
          updateSaveUI(false);
      }

      start_audio((fb) => {
          if (useWebGpu) { render_frame_wgpu(fb); } else { render2d(ctx2d, fb); }
      });
  }
  ```

  Update the `rom-picker` `onload` callback and the sample-ROM fetch handler to `await loadRomBytes(...)` (make their enclosing callbacks `async`). *(req 1–4)*

- [ ] 8. In `www/index.js`, add `updateSaveUI` and the three menu-item handlers (plus `sav-picker`) inside `main()`, after the zoom-button block:

  ```js
  function updateSaveUI(hasSram) {
      const show = hasSram ? '' : 'none';
      document.getElementById('menu-save-sep').style.display = show;
      document.getElementById('menu-save').style.display = show;
      document.getElementById('menu-export-sav').style.display = show;
      document.getElementById('menu-import-sav').style.display = show;
  }

  document.getElementById('menu-save').addEventListener('click', async () => {
      if (!currentChecksum) return;
      await saveSramIdb(currentChecksum, get_sram());
      const btn = document.getElementById('menu-save');
      btn.textContent = 'Saved!';
      setTimeout(() => { btn.textContent = 'Save game'; }, 1000);
  });

  document.getElementById('menu-export-sav').addEventListener('click', () => {
      const blob = new Blob([get_sram()], { type: 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      const a = Object.assign(document.createElement('a'), {
          href: url, download: `${rom_title() || 'save'}.sav`
      });
      a.click();
      URL.revokeObjectURL(url);
      gbMenu.classList.add('hidden');
  });

  document.getElementById('menu-import-sav').addEventListener('click', () => {
      document.getElementById('sav-picker').click();
      gbMenu.classList.add('hidden');
  });

  document.getElementById('sav-picker').addEventListener('change', async (e) => {
      const file = e.target.files[0];
      if (!file) return;
      const data = new Uint8Array(await file.arrayBuffer());
      set_sram(data);
      if (currentChecksum) await saveSramIdb(currentChecksum, data);
      e.target.value = '';
  });
  ```
  *(req 3, 5, 6)*

## Manual Testing

1. Run `./preflight.sh`. Confirm PASS.
2. Build and serve: `wasm-pack build crates/gpuboy-wasm --target web --out-dir ../../pkg && python -m http.server 8000`.
3. Open `http://localhost:8000/www/`. Load a ROM with SRAM (any MBC1/3/5 game that saves).
4. Confirm the hamburger menu shows "Save game", "Export .sav", "Import save".
5. Play briefly. Wait 30 seconds. Hard-reload the page (`Ctrl+Shift+R`). Load the same ROM. Confirm the save was restored (player position / progress preserved).
6. Load the same ROM again. Click "Save game". Reload immediately. Confirm progress is still restored (manual save worked).
7. Click "Export .sav". Confirm a `.sav` file downloads with the correct name and a non-zero byte count.
8. Load a known-good `.sav` via "Import save". Confirm the emulator state updates (character position changes, etc.). Reload and reload the ROM to confirm IDB was also updated.
9. Load a ROM with no SRAM (e.g. Tetris). Confirm save menu items are hidden.
10. While Pokémon (with SRAM) is running, switch to Tetris (no SRAM). Reload and reload Pokémon. Confirm Pokémon's save was preserved when switching away.

**Green light:** [ ]
