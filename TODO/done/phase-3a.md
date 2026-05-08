# Phase 3a: PPU Core

## Overview

Implements the Game Boy PPU (Pixel Processing Unit) as pure emulation logic in `gpuboy-core`. The PPU draws 144 visible scanlines per frame using a scanline-based renderer (not a hardware FIFO), producing a flat `[u8; 160 * 144 * 4]` RGBA framebuffer. VRAM, OAM, and all PPU IO registers are added to the Bus. The WASM layer exposes `get_framebuffer()` and the JS shell displays the result via a 2D canvas `putImageData` call — the simplest possible display path. Phase 3b will replace that path with WebGPU; Phase 3a's goal is PPU correctness with a minimal renderer.

## Requirements

1. WHEN the emulator runs, THEN the PPU advances in lockstep with the CPU: for every T-cycle the CPU consumes, the PPU advances the same number of T-cycles.

2. WHEN the PPU is rendering a visible scanline (LY 0–143), THEN it cycles through Mode 2 (OAM scan, 80 T-cycles), Mode 3 (pixel transfer, 172 T-cycles), and Mode 0 (HBlank, 204 T-cycles) in that order, for a total of 456 T-cycles per scanline.

3. WHEN LY reaches 144, THEN the PPU enters Mode 1 (VBlank), bit 0 of IF is set (VBlank interrupt), and VBlank continues for 10 scanlines (LY 144–153, 456 T-cycles each) before LY wraps back to 0.

4. WHEN the PPU enters a new mode and the corresponding STAT interrupt enable bit is set (bit 5 for Mode 2, bit 4 for Mode 1/VBlank, bit 3 for Mode 0/HBlank), THEN bit 1 of IF is set (STAT interrupt).

5. WHEN LY equals LYC, THEN STAT bit 2 (coincidence flag) is set; if STAT bit 6 (LYC=LY interrupt enable) is also set, THEN bit 1 of IF is set.

6. WHEN the PPU renders a visible scanline, THEN if LCDC bit 0 is set the background layer is rendered: each pixel's color index is looked up from the BG tile map and tile data using SCX/SCY scroll offsets, then translated through the BGP palette register to an RGBA value written to the framebuffer.

7. WHEN the PPU renders a visible scanline and LCDC bit 5 is set and WY <= current LY, THEN the window layer is drawn over background pixels starting at screen X position max(0, WX-7), using the window tile map and the same tile data addressing as BG.

8. WHEN the PPU renders a visible scanline and LCDC bit 1 is set, THEN up to 10 sprites that overlap the current scanline are collected from OAM and drawn on top of BG/window, respecting per-sprite X-flip, Y-flip, palette (OBP0/OBP1), and priority (LCDC bit 7 of sprite attributes: when set, sprite is behind non-zero BG colors).

9. WHEN a write is made to 0xFF46 (OAM DMA), THEN 160 bytes are immediately copied from address `(value << 8)` in the Bus memory map to OAM (0xFE00–0xFE9F). No CPU stall is modeled in this phase.

10. WHEN `get_framebuffer()` is called, THEN it returns the pixel data from the most recently completed frame as a flat `[u8; 160 * 144 * 4]` RGBA byte slice, row-major, with pixel (x, y) at offset `(y * 160 + x) * 4`.

11. WHEN the WASM `get_framebuffer()` export is called from JavaScript, THEN it returns the framebuffer as a `Uint8Array`-compatible `Vec<u8>` and JavaScript can create an `ImageData` from it and draw it to a 2D canvas.

## Acceptance Criteria

- [ ] `cargo test -p gpuboy-core` passes all unit tests including new PPU tests
- [ ] PPU mode sequence: starting from LY=0, after 80 T-cycles mode=2→3, after 172 more mode=3→0, after 204 more LY increments and mode=0→2 (or Mode 1 at LY=144)
- [ ] VBlank interrupt: after 144 × 456 T-cycles from LY=0, IF bit 0 is set and LY=144
- [ ] STAT LYC=LY interrupt: set LYC=5; after enough T-cycles for LY to reach 5, IF bit 1 is set (when STAT bit 6 is enabled)
- [ ] BG rendering: a ROM with known tile data and tile map produces correctly scrolled background pixels in the framebuffer
- [ ] Sprite rendering: a sprite in OAM at known X/Y position appears at the correct framebuffer coordinates with correct palette color
- [ ] OAM DMA: writing 0xC0 to 0xFF46 copies 160 bytes from 0xC000 to OAM
- [ ] `get_framebuffer()` in WASM returns a Vec of length 160 × 144 × 4 = 92160
- [ ] 2D canvas path in `www/index.js` displays the framebuffer; loading a ROM and calling `step_frame()` shows something on screen (at minimum, the correct background fill for the ROM)
- [ ] No regressions from Phase 2 (all Phase 2 unit tests still pass)

## Design

### Architecture

```
gpuboy-core/src/
  lib.rs        — Emulator adds get_framebuffer(); step() now also calls bus.step_ppu()
  ppu.rs        — Ppu struct; step(t_cycles, if_reg); scanline renderer; framebuffer
  bus.rs        — adds Ppu, vram [u8; 0x2000], oam [u8; 0xA0], PPU IO regs; OAM DMA
  cpu.rs        — unchanged
  timer.rs      — unchanged
  cartridge.rs  — unchanged
gpuboy-wasm/src/
  lib.rs        — adds get_framebuffer() WASM export
www/
  index.js      — after step_frame(), call get_framebuffer(), draw via putImageData
  index.html    — canvas sized 160×144, CSS scaled 3×
```

`Ppu` is owned by `Bus` (mirrors how `Timer` is owned by `Bus`). After each `Cpu::step`, `Emulator::step` calls `bus.step_ppu(t_cycles)`, which delegates to `ppu.step(t_cycles, &mut interrupt_flags)`.

### Data Structures

**Ppu**

```rust
pub struct Ppu {
    // Registers (also stored in Bus IO map for CPU read/write; Bus syncs them into Ppu)
    pub lcdc: u8,   // 0xFF40
    pub stat: u8,   // 0xFF41 — bits 6-3 R/W; bits 2-0 read-only (written by PPU)
    pub scy: u8,    // 0xFF42
    pub scx: u8,    // 0xFF43
    pub ly: u8,     // 0xFF44 — written by PPU only
    pub lyc: u8,    // 0xFF45
    pub bgp: u8,    // 0xFF47
    pub obp0: u8,   // 0xFF48
    pub obp1: u8,   // 0xFF49
    pub wy: u8,     // 0xFF4A
    pub wx: u8,     // 0xFF4B

    // Internal state
    dot: u32,                       // T-cycles into the current scanline (0–455)
    mode: u8,                       // 0, 1, 2, 3
    window_line: u8,                // internal window line counter (increments when window row drawn)
    framebuffer: [u8; 160 * 144 * 4],
    back_buffer: [u8; 160 * 144 * 4], // completed frame; swapped at VBlank
}
```

Palette decoding: `palette_color(palette: u8, idx: u8) -> [u8; 4]`. Extracts bits `(palette >> (idx * 2)) & 0x03` and maps:
- 0 → `[0xE0, 0xF8, 0xD0, 0xFF]` (lightest green)
- 1 → `[0x88, 0xC0, 0x70, 0xFF]`
- 2 → `[0x34, 0x68, 0x56, 0xFF]`
- 3 → `[0x08, 0x18, 0x20, 0xFF]` (darkest)

**Bus additions**

```rust
pub struct Bus {
    // existing fields ...
    // new:
    pub ppu: Ppu,
    vram: [u8; 0x2000],  // 0x8000–0x9FFF
    oam:  [u8; 0xA0],    // 0xFE00–0xFE9F
}
```

Bus `read`/`write` routes:
- `0x8000..=0x9FFF` → vram
- `0xFE00..=0xFE9F` → oam
- `0xFF40..=0xFF4B` → PPU register read/write (see IO Map below)

**Sprite candidate (local to `render_scanline`)**

```rust
struct Sprite { x: u8, y: u8, tile: u8, attrs: u8 }
```

Collected by scanning all 40 OAM entries; keep the first 10 that overlap the current LY.

`render_scanline` allocates a `bg_indices: [u8; 160]` stack array for BG color indices needed for sprite priority compositing.

### Key Decisions

**Scanline renderer, not hardware FIFO.** The real PPU uses a pixel FIFO with complex pause behavior (sprite fetches stall the FIFO). A scanline renderer renders each layer in a single pass per scanline and composites them. It produces correct output for the vast majority of games, requires a fraction of the code, and makes PPU correctness easy to test. The FIFO is deferred indefinitely.

**Mode 3 fixed at 172 T-cycles.** Hardware Mode 3 takes 172–289 T-cycles depending on sprite count and SCX fine-scroll. Fixing it at 172 keeps the mode state machine simple (HBlank = 204 T-cycles, total = 456). This is the shortest legal Mode 3. Timing-sensitive demos may glitch; games are generally unaffected.

**Scanline rendered at Mode 3 entry.** The scanline is composited and written to the framebuffer when Mode 3 begins (at dot 80). This is slightly early vs hardware but consistent and simple.

**Double-buffer for framebuffer.** `framebuffer` is written during frame rendering. At VBlank start, `back_buffer` is swapped in as the completed frame and exposed to `get_framebuffer()`. This ensures the JS side always reads a complete, stable frame.

**VRAM/OAM always accessible.** Real hardware blocks CPU access to VRAM during Mode 3 and OAM during Modes 2 and 3. Phase 3a skips this; CPU always reads/writes VRAM and OAM directly. Blocking is deferred to a future phase.

**OAM DMA is instantaneous.** Real DMA takes 160 M-cycles (640 T-cycles) and blocks the CPU from all memory except HRAM. Phase 3a copies immediately with no CPU stall. Most games write a wait loop during DMA anyway, so this works in practice.

**8×16 sprites deferred.** LCDC bit 2 (sprite size 8×16) is recognized but sprites are always rendered as 8×8. Note this in the spec; add 8×16 support in a future phase.

**PPU disabled (LCDC bit 7 = 0).** When LCD is off, the PPU does not advance dot/LY, mode is forced to 0, and the framebuffer is filled with white. This allows games that disable the LCD during OAM writes to work correctly.

### Bus IO Map (additions)

| Address      | Register | R/W | Notes |
|--------------|----------|-----|-------|
| `0x8000–0x9FFF` | VRAM  | R/W | 8 KB; via `vram[]` |
| `0xFE00–0xFE9F` | OAM   | R/W | 160 bytes; via `oam[]` |
| `0xFF40` | LCDC | R/W | LCD control |
| `0xFF41` | STAT | R/W | bits 6-3 R/W; bits 2-0 read-only, written by PPU |
| `0xFF42` | SCY  | R/W | BG scroll Y |
| `0xFF43` | SCX  | R/W | BG scroll X |
| `0xFF44` | LY   | R   | Current scanline; writes ignored |
| `0xFF45` | LYC  | R/W | LY compare |
| `0xFF46` | DMA  | W   | OAM DMA source (high byte); triggers copy |
| `0xFF47` | BGP  | R/W | BG palette |
| `0xFF48` | OBP0 | R/W | Sprite palette 0 |
| `0xFF49` | OBP1 | R/W | Sprite palette 1 |
| `0xFF4A` | WY   | R/W | Window Y |
| `0xFF4B` | WX   | R/W | Window X |

PPU registers are stored canonically in `Bus::ppu` (the `Ppu` struct fields). Bus read/write for the PPU register range delegates to getters/setters on `Ppu`. STAT writes must preserve bits 2-0 (PPU-owned): `ppu.stat = (ppu.stat & 0x07) | (val & 0xF8)`.

### Scanline Rendering Algorithm

`Ppu::render_scanline(vram: &[u8], oam: &[u8])` — called at dot 80 (Mode 3 entry) for LY 0–143.

1. **Background layer**:
   - If LCDC bit 0 is 0, fill all 160 pixels of this scanline with palette color index 0 (apply `palette_color(bgp, 0)` for each pixel) and set `bg_indices[x] = 0` for all x. If LCDC bit 0 is set, proceed with normal BG rendering:
   - For each screen X (0–159):
     - Effective BG X = (SCX + x) % 256, effective BG Y = (SCY + LY) % 256
     - Tile map address: `map_base + (bg_y / 8) * 32 + (bg_x / 8)` where `map_base` = `0x9C00` if LCDC bit 3 set, else `0x9800`
     - Tile index from vram; tile data base: if LCDC bit 4 set → `0x8000 + tile_idx * 16`, else → `0x9000 + (tile_idx as i8 as i16 * 16)` (signed addressing from 0x9000)
     - Row within tile: `bg_y % 8`; column: `bg_x % 8`
     - Low byte = `vram[tile_base + row * 2]`, high byte = `vram[tile_base + row * 2 + 1]`
     - Color index: bit `(7 - col)` of low byte = bit 0 of index; same bit of high byte = bit 1
     - Store color index in `bg_indices[x]`; apply BGP palette; write RGBA to framebuffer

2. **Window layer** (if LCDC bit 5 set and `wy <= ly` and `wx <= 166`):
   - Window is drawn for screen X >= `wx.saturating_sub(7)`
   - Window has its own internal line counter (`window_line`), reset to 0 when LCD turns on; incremented each scanline the window is visible
   - Tile map: `0x9C00` if LCDC bit 6 set, else `0x9800`
   - Same tile data addressing and pixel extraction as BG; store color index in `bg_indices[x]`; replace framebuffer pixel

3. **Sprite layer** (if LCDC bit 1 set):
   - Scan OAM (40 sprites, 4 bytes each); collect sprites where `ly + 16 >= sprite_y && ly + 16 < sprite_y + 8` (always 8 for Phase 3a). Keep first 10 (hardware limit).
   - Sort by X coordinate ascending (lower X = higher priority in case of overlap); stable.
   - For each collected sprite, for each pixel in its horizontal extent that falls within screen X 0–159:
     - `sx = sprite_x - 8 + col` (sprite_x is OAM byte 1)
     - If `sx < 0 || sx >= 160`: skip
     - Tile row: `ly + 16 - sprite_y`; if Y-flip (attrs bit 6): `7 - row`
     - Tile col: `col`; if X-flip (attrs bit 5): `7 - col`
     - Fetch pixel from tile data at `0x8000 + tile_idx * 16 + row * 2` (sprites always use 0x8000 addressing)
     - Color index 0 = transparent; skip
     - Palette: attrs bit 4 → OBP1, else OBP0
     - Priority (attrs bit 7): if set, only draw the sprite pixel if `bg_indices[sx] == 0`; else always draw over BG
     - Write RGBA to framebuffer

## Tasks

- [ ] 1. Create `crates/gpuboy-core/src/ppu.rs`. Define `pub struct Ppu` with all fields listed in Design §Data Structures. Implement `Ppu::new()` initializing `lcdc=0x91` (LCD on, BG on, BG tile data at 0x8000), `stat=0x82` (bit 7 always reads 1, Mode 2), `bgp=0xFC`, `ly=0`, `dot=0`, `mode=2` (post-boot state is Mode 2: OAM scan at LY=0), all other regs to 0, framebuffer and back_buffer to all white (`0xFF`). *(req 1)*

- [ ] 2. Add `pub mod ppu;` to `crates/gpuboy-core/src/lib.rs`. *(req 1)*

- [ ] 3. In `ppu.rs`, implement `fn palette_color(palette: u8, idx: u8) -> [u8; 4]` mapping color indices 0–3 to the four DMG RGBA values defined in Design §Data Structures. *(req 6)*

- [ ] 4. In `ppu.rs`, implement `fn tile_pixel(vram: &[u8], tile_base: usize, row: usize, col: usize) -> u8` that extracts the 2-bit color index for a given tile row and column from VRAM tile data (low byte bit = index bit 0, high byte bit = index bit 1). *(req 6, 7, 8)*

- [ ] 5. In `ppu.rs`, implement `Ppu::render_scanline(&mut self, vram: &[u8], oam: &[u8])` following the algorithm in Design §Scanline Rendering Algorithm exactly: BG layer, window layer (with `window_line` tracking), sprite layer (10-sprite limit, X-sorted, X/Y-flip, palette, priority). Allocate a `bg_indices: [u8; 160]` stack array at the start of `render_scanline` to hold the raw BG/window color index (0–3) for each pixel in this scanline. During BG and window rendering, write the color index to `bg_indices[x]` alongside writing the RGBA to the framebuffer. During sprite compositing, use `bg_indices[sx]` to check BG priority: when sprite attrs bit 7 = 1, only draw the sprite pixel if `bg_indices[sx] == 0`. If LCDC bit 0 = 0, fill the scanline pixels with `palette_color(self.bgp, 0)` and set all `bg_indices` to 0, then skip to window/sprite rendering. *(req 6, 7, 8)*

- [ ] 6. In `ppu.rs`, implement `pub fn step(&mut self, t_cycles: u32, vram: &[u8], oam: &[u8], interrupt_flags: &mut u8)`. Process T-cycles one scanline at a time:
    - If LCDC bit 7 is 0 (LCD off): fill framebuffer with white, set `ly=0`, `dot=0`, `mode=0`; return immediately.
    - Accumulate `dot += t_cycles`. While `dot >= 456`: `dot -= 456`; advance LY (see below).
    - Within a scanline, determine mode from `dot`:
        - dot < 80: Mode 2 (OAM scan)
        - 80 <= dot < 252: Mode 3 (pixel transfer); when `dot` crosses 80 and `mode` is 2 (i.e., this is the Mode 2→3 transition), set `mode = 3`, call `render_scanline(vram, oam)` once, and fire the STAT Mode 3 interrupt if STAT bit 3 is set. Do not call `render_scanline` again for this scanline.
        - 252 <= dot < 456: Mode 0 (HBlank)
    - On mode transitions, update `stat` bits 1-0 and fire STAT interrupt if the corresponding enable bit is set.
    - When LY increments:
        - At LY=144: set mode to 1 (VBlank); swap framebuffers (`back_buffer = framebuffer`); set IF bit 0 (VBlank); fire STAT Mode 1 interrupt if STAT bit 4 set.
        - When LY would increment past 153, reset LY to 0 instead (LY is always 0–153); also reset `window_line` to 0 at this point.
    - After each LY change: update STAT bit 2 (LYC=LY flag); if LY==LYC and STAT bit 6 set: set IF bit 1 (STAT).
    - Note: `render_scanline` is responsible for incrementing `self.window_line` when it draws a window row (see Task 5). Task 6 must also ensure `window_line` is reset to 0 when `ly` wraps to 0. *(req 1, 2, 3, 4, 5)*

- [ ] 7. Add `pub fn get_framebuffer(&self) -> &[u8]` to `Ppu` returning `&self.back_buffer`. *(req 10)*

- [ ] 8. Update `crates/gpuboy-core/src/bus.rs`: add `pub ppu: Ppu`, `vram: [u8; 0x2000]`, `oam: [u8; 0xA0]` to `Bus`. Update `Bus::new` to initialize them. Update `read` to handle `0x8000..=0x9FFF` → `vram[addr - 0x8000]`, `0xFE00..=0xFE9F` → `oam[addr - 0xFE00]`, and `0xFF40..=0xFF4B` (PPU register reads per IO map above — return `ppu.ly` for 0xFF44, `ppu.stat | 0x80` for 0xFF41 (STAT reads must always set bit 7), etc.; return 0xFF for 0xFF46 DMA which is write-only). Update `write` to handle the same ranges. For STAT writes: `ppu.stat = (ppu.stat & 0x07) | (val & 0xF8)` (note: bit 7 is not stored from writes; compensate by always setting it on reads via `ppu.stat | 0x80`). For LY writes: ignore. For 0xFF46 DMA writes: the borrow checker will not allow simultaneously reading `self` via `self.read()` and writing `self.oam`; use a temporary buffer instead:
    ```rust
    let src = (val as u16) << 8;
    let mut buf = [0u8; 160];
    for i in 0..160u16 {
        buf[i as usize] = self.read(src + i);
    }
    self.oam.copy_from_slice(&buf);
    ```
    *(req 6, 7, 8, 9)*

- [ ] 9. Add `pub fn step_ppu(&mut self, t_cycles: u32)` to `Bus`. Rust cannot simultaneously hold `&mut self.ppu` and `&self.vram`/`&self.oam`/`&mut self.interrupt_flags` through a single inline call on `self`. Use struct destructuring to split the `&mut self` borrow into disjoint field borrows, which the borrow checker allows:
    ```rust
    pub fn step_ppu(&mut self, t_cycles: u32) {
        let Bus { ppu, vram, oam, interrupt_flags, .. } = self;
        ppu.step(t_cycles, vram, oam, interrupt_flags);
    }
    ```
    *(req 1)*

- [ ] 10. Update `crates/gpuboy-core/src/lib.rs` (`Emulator::step`): after calling `bus.step_timer(t_cycles)`, also call `bus.step_ppu(t_cycles)`. Add `pub fn get_framebuffer(&self) -> &[u8]` to `Emulator` delegating to `self.bus.ppu.get_framebuffer()`. *(req 1, 10)*

- [ ] 11. Update `crates/gpuboy-wasm/src/lib.rs`: add `#[wasm_bindgen] pub fn get_framebuffer() -> Vec<u8>` that borrows EMULATOR and returns the framebuffer as a `Vec<u8>` (or an empty vec if no emulator loaded). Note: the `&[u8]` returned by `emu.get_framebuffer()` cannot escape the `EMULATOR.with(...)` closure; call `.to_vec()` inside the closure before returning. *(req 11)*

- [ ] 12. Update `www/index.html`: ensure `<canvas id="screen" width="160" height="144" style="width:480px;height:432px;image-rendering:pixelated;display:block;"></canvas>` exists (update if dimensions differ from Phase 2). Add `<div id="error" style="display:none;color:red;"></div>` for Phase 3b fallback wiring. *(req 11)*

- [ ] 13. Update `www/index.js`: after `step_frame()`, call `get_framebuffer()` to get a `Uint8Array` of length 92160, create `new ImageData(new Uint8ClampedArray(fb), 160, 144)`, acquire the 2D context from `#screen`, and call `ctx.putImageData(imageData, 0, 0)`. Wrap this in `requestAnimationFrame` to run continuously after a ROM is loaded. Start the `requestAnimationFrame` loop inside the `FileReader.onload` callback, after `load_rom(data)` returns without error. Use a module-level `let animationId = null` guard so the loop is only started once — do not start a second loop if another ROM is loaded while one is already running; cancel the existing loop first with `cancelAnimationFrame(animationId)`. *(req 11)*

- [ ] 14. Add unit tests to `crates/gpuboy-core/src/ppu.rs`:
    - `test_mode_sequence`: create a `Ppu`, call `step` enough times to verify mode transitions: after 80 T-cycles at LY=0 → mode=3; after 252 more → mode=0; after 204 more → LY=1, mode=2.
    - `test_vblank_interrupt`: step through 144 × 456 T-cycles; verify that `interrupt_flags & 0x01 != 0` (VBlank) and `ly == 144`.
    - `test_lyc_stat_interrupt`: set `ppu.lyc = 5`, `ppu.stat |= 0x40` (enable LYC=LY interrupt); step to LY=5; verify `interrupt_flags & 0x02 != 0`.
    - `test_bg_rendering`: construct minimal VRAM with a known tile (checkerboard) at tile index 0, set tile map byte 0 to 0, BGP=0b11100100 (identity), SCX=SCY=0, LCDC=0x91; call `render_scanline`; verify pixel (0,0) RGBA matches color index expected from tile data.
    - `test_sprite_rendering`: place a sprite in OAM at Y=16, X=8 (screen position 0,0), tile=0, attrs=0; put known tile data in VRAM at 0x8000; call `render_scanline`; verify pixel (0,0) matches the sprite's non-transparent color.
    *(req 1, 2, 3, 4, 5, 6, 8)*

- [ ] 15. Add a unit test to `crates/gpuboy-core/src/bus.rs`:
    - `test_oam_dma`: write 160 distinct bytes to WRAM starting at 0xC000; write 0xC0 to Bus address 0xFF46; read OAM bytes 0xFE00–0xFE9F; verify they match. *(req 9)*

## Manual Testing

1. Run `cargo test -p gpuboy-core` and confirm all tests pass.
2. Run `cargo clippy -p gpuboy-core -- -D warnings` and confirm no warnings.
3. Build WASM: `wasm-pack build crates/gpuboy-wasm --target web`.
4. Serve locally: `python -m http.server 8000`, open `http://localhost:8000/www/`.
5. Open DevTools console. Confirm "gpuboy ready" appears with no errors.
6. Load a flat (MBC-0) .gb ROM. Confirm the ROM title appears in the console.
7. Confirm game pixels appear on the canvas — at minimum a background color or title screen should be visible. The canvas should be pixel-sharp (no blurring) due to `image-rendering: pixelated`.
8. Confirm the animation loop runs smoothly (no JS exceptions, frame counter advancing in console if logged).
9. Load a ROM known to produce a visible title screen (e.g. Tetris, Dr. Mario). Confirm the title screen is recognizable and not corrupted.
10. Open DevTools > Console, call `get_framebuffer()` from the JS console and confirm it returns a Uint8Array of length 92160.

**Green light:** [ ]
