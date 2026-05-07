# Phase 1: Memory Bus + ROM Loading

## Overview

Introduce `gpuboy-core` as the emulator logic crate and implement the Game Boy memory bus and ROM loading. At the end of this phase, a flat ROM (no MBC) can be loaded from the browser, and the address space is fully mapped — ready for the CPU to start executing in Phase 2. No emulation runs yet; this phase is purely about wiring the data path.

## Requirements

1. WHEN a ROM file is provided, THEN `gpuboy-core` parses the cartridge header and exposes title, cartridge type, ROM size, and RAM size.
2. WHEN a flat ROM (MBC type 0x00) is loaded, THEN reads from 0x0000–0x7FFF return the correct byte from the ROM, or 0xFF if the address exceeds the ROM's length.
3. WHEN a write is issued to ROM space (0x0000–0x7FFF), THEN the write is silently ignored (no panic).
4. WHEN a read or write targets Work RAM (0xC000–0xDFFF) or echo RAM (0xE000–0xFDFF), THEN the operation hits the 8 KiB WRAM array.
5. WHEN a read or write targets High RAM (0xFF80–0xFFFE), THEN the operation hits the 127-byte HRAM array.
6. WHEN a read or write targets an unmapped or unimplemented address, THEN it returns 0xFF on read and silently drops on write (open-bus behavior).
7. WHEN a ROM is too small to contain a valid header (< 0x014A bytes) or has an unsupported MBC type, THEN `gpuboy-core` returns a descriptive error string rather than panicking.
8. WHEN the browser loads a ROM file via the file picker, THEN the ROM bytes are passed to `gpuboy-core` and the cartridge title is logged to the console on success, or the error string is logged on failure.

## Acceptance Criteria

- [ ] `CartridgeHeader::parse` correctly parses title, type, ROM size, and RAM size from a real ROM fixture (values verified against raw bytes)
- [ ] Bus reads from 0x0000–0x7FFF return expected bytes for a flat ROM
- [ ] Bus reads from WRAM (0xC000) after a WRAM write return the written value
- [ ] Bus reads from HRAM (0xFF80) after an HRAM write return the written value
- [ ] Reads from unmapped addresses return 0xFF
- [ ] Loading a too-small ROM (< 0x014A bytes) logs an error containing "ROM too small"
- [ ] Loading an MBC1 ROM logs an error containing "unsupported MBC"
- [ ] Browser: loading a flat ROM via the file picker logs the cartridge title (e.g. `TETRIS`)
- [ ] Browser: file read errors (e.g. cancelled read) are caught and logged, not silently swallowed
- [ ] `cargo test` passes: all unit tests in `cartridge.rs` and `bus.rs` green
- [ ] `cargo clippy -- -D warnings` passes with no warnings
- [ ] No regressions: "gpuboy ready" still appears on page load

## Design

### Architecture

Add `gpuboy-core` as a new workspace crate. `gpuboy-wasm` depends on it and stays thin — it owns the JS boundary, file picker glue, and console logging. All emulator logic lives in `gpuboy-core`.

```
crates/
  gpuboy-wasm/      # WASM boundary — gains file picker + passes ROM bytes to core
  gpuboy-core/      # new: cartridge, bus
    src/
      lib.rs        # re-exports
      cartridge.rs  # header parsing + ROM storage
      bus.rs        # address decode + read/write
```

`Cargo.toml` workspace `members` gains `"crates/gpuboy-core"`.

### Data Structures

```rust
// cartridge.rs
pub struct CartridgeHeader {
    pub title: String,       // bytes 0x0134–0x0143, trimmed at first \0 or non-ASCII
    pub cartridge_type: u8,  // byte 0x0147
    pub rom_size: u8,        // byte 0x0148
    pub ram_size: u8,        // byte 0x0149
}

impl CartridgeHeader {
    pub fn parse(rom: &[u8]) -> Result<Self, String>  // length check + field extraction, no MBC check
}

pub struct Cartridge {
    pub header: CartridgeHeader,
    rom: Vec<u8>,
}

impl Cartridge {
    pub fn load(rom: Vec<u8>) -> Result<Self, String>  // calls CartridgeHeader::parse, then rejects unsupported MBC
    pub fn read(&self, addr: u16) -> u8                // bounds-safe: returns 0xFF past end of ROM
}

// bus.rs
pub struct Bus {
    cartridge: Cartridge,
    wram: [u8; 0x2000],   // 8 KiB, 0xC000–0xDFFF
    hram: [u8; 0x7F],     // 127 bytes, 0xFF80–0xFFFE
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self
    pub fn read(&self, addr: u16) -> u8
    pub fn write(&mut self, addr: u16, val: u8)
}
```

### Key Decisions

- **Flat ROM only (MBC type 0x00)**: MBC banking is Phase 6. Returning an error for other types now prevents silently wrong behavior later.
- **`Vec<u8>` for ROM storage**: ROM size varies (32 KiB–8 MiB). Stack allocation isn't viable; heap is fine.
- **`[u8; N]` for WRAM/HRAM**: Fixed size, stack-allocated, zero-initialized. Simpler than Vec for regions with known bounds.
- **`Result<_, String>` for load errors**: Keeps `gpuboy-core` free of error-crate dependencies in Phase 1. Can be refined later.
- **Title parsing**: Read bytes 0x0134–0x0143 (16 bytes), trim at the first `\0` or first non-printable-ASCII byte. This handles both old DMG-only ROMs (full 16-byte title) and CGB-capable ROMs where 0x013F–0x0143 overlap the manufacturer code and CGB flag.
- **Echo RAM (0xE000–0xFDFF) mirrors WRAM**: Indexed as `wram[(addr - 0xE000) as usize]`. This range covers 0x1E00 bytes, staying within WRAM's 0x2000.
- **Open-bus returns 0xFF**: Standard Game Boy behavior for reads from unmapped addresses. In Phase 1 this covers: VRAM (0x8000–0x9FFF), external RAM (0xA000–0xBFFF), OAM (0xFE00–0xFE9F), prohibited (0xFEA0–0xFEFF), I/O registers (0xFF00–0xFF7F), and the IE register (0xFFFF). These will gain real implementations in later phases.
- **File reading done in JS**: The JS `FileReader` API converts the file to an `ArrayBuffer`; `index.js` wraps it in a `Uint8Array` and passes it to `load_rom(data: Vec<u8>)`. wasm-bindgen handles the copy automatically. No web-sys DOM types are needed on the Rust side beyond `console`.

## Tasks

- [ ] 1. Add `"crates/gpuboy-core"` to `members` in root `Cargo.toml`. Create `crates/gpuboy-core/Cargo.toml`: lib crate, edition 2021, no external deps, `[lints] workspace = true`. *(housekeeping)*
- [ ] 2. Create `crates/gpuboy-core/src/lib.rs`: `pub mod cartridge; pub mod bus;`. *(housekeeping)*
- [ ] 3. Implement `crates/gpuboy-core/src/cartridge.rs`:
  - `CartridgeHeader` with fields `title: String`, `cartridge_type: u8`, `rom_size: u8`, `ram_size: u8`.
  - `CartridgeHeader::parse(rom: &[u8]) -> Result<Self, String>`: check `rom.len() >= 0x014A` else `Err("ROM too small".into())`; parse title from bytes 0x0134–0x0143 truncating at the first `\0` or first byte where `!b.is_ascii_graphic() && b != b' '`; read `cartridge_type = rom[0x0147]`, `rom_size = rom[0x0148]`, `ram_size = rom[0x0149]`. No MBC check.
  - `Cartridge` with `pub header: CartridgeHeader` and `rom: Vec<u8>`.
  - `Cartridge::load(rom: Vec<u8>) -> Result<Self, String>`: call `CartridgeHeader::parse(&rom)` to get the header, then return `Err(format!("unsupported MBC: 0x{:02X}", header.cartridge_type))` if `cartridge_type != 0x00`, else return `Ok(Cartridge { header, rom })`.
  - `Cartridge::read(&self, addr: u16) -> u8`: `self.rom.get(addr as usize).copied().unwrap_or(0xFF)`.
  *(req 1, 2, 7)*
- [ ] 4. Implement `crates/gpuboy-core/src/bus.rs`:
  - `Bus` struct as specified above.
  - `Bus::new(cartridge: Cartridge) -> Self`: zero-initialize WRAM and HRAM.
  - `Bus::read(&self, addr: u16) -> u8`: dispatch on address — ROM 0x0000–0x7FFF → `self.cartridge.read(addr)`; WRAM 0xC000–0xDFFF → `self.wram[(addr - 0xC000) as usize]`; echo RAM 0xE000–0xFDFF → `self.wram[(addr - 0xE000) as usize]`; HRAM 0xFF80–0xFFFE → `self.hram[(addr - 0xFF80) as usize]`; all other addresses → `0xFF`.
  - `Bus::write(&mut self, addr: u16, val: u8)`: dispatch on address — ROM 0x0000–0x7FFF → silent no-op; WRAM 0xC000–0xDFFF → `self.wram[(addr - 0xC000) as usize] = val`; echo RAM 0xE000–0xFDFF → `self.wram[(addr - 0xE000) as usize] = val`; HRAM 0xFF80–0xFFFE → `self.hram[(addr - 0xFF80) as usize] = val`; all other addresses → silent no-op.
  *(req 2, 3, 4, 5, 6)*
- [ ] 5. Add `gpuboy-core = { path = "../gpuboy-core" }` to `[dependencies]` in `crates/gpuboy-wasm/Cargo.toml`. *(housekeeping)*
- [ ] 6. Update `crates/gpuboy-wasm/src/lib.rs`: add `use gpuboy_core::cartridge::Cartridge;`. Add `#[wasm_bindgen] pub fn load_rom(data: Vec<u8>)` that calls `Cartridge::load(data)` — on `Ok(cart)` logs `cart.header.title` via `web_sys::console::log_1`; on `Err(e)` logs the error string via `web_sys::console::log_1`. *(req 7, 8)*
- [ ] 7. Update `www/index.html`: add `<input type="file" id="rom-picker" accept=".gb,.gb">` above the canvas. *(req 8)*
- [ ] 8. Update `www/index.js`: after `run()`, wire up `#rom-picker`'s `change` event. Create a `FileReader`, set `onerror` to log `"FileReader error: " + e.target.error` to console, set `onload` to check `e.target.result instanceof ArrayBuffer` then call `load_rom(new Uint8Array(e.target.result))`. Call `reader.readAsArrayBuffer(e.target.files[0])`. *(req 8)*
- [ ] 9. Set up test ROM infrastructure. *(housekeeping)*
  - Add `tests/roms/` to `.gitignore`.
  - Create `scripts/download-test-roms.sh`: checks if `tests/roms/` already exists and is non-empty (skip if so); otherwise downloads `https://github.com/c-sp/game-boy-test-roms/releases/download/v7.0/game-boy-test-roms-v7.0.zip` to a temp file, unzips into `tests/roms/`, removes the zip. Make the script executable.

- [ ] 10. Add `#[cfg(test)]` modules to `cartridge.rs` and `bus.rs`. *(req 1, 2, 3, 4, 5, 6, 7)*

  > **Implementation note:** Before writing these tests, run `scripts/download-test-roms.sh` and inspect the extracted `tests/roms/` tree to find the correct paths. Then read the raw header bytes of your chosen fixture (`xxd tests/roms/<path> | head -6`) to hardcode exact expected values for `title`, `cartridge_type`, `rom_size`, and `ram_size`. ROM paths and expected values are intentionally left as TBD here — fill them in during implementation. If no flat ROM (MBC 0x00) exists in the bundle, fall back to `minimal_rom(0x00)` for the `load_flat_rom` test only.

  **cartridge.rs** — uses real ROM fixtures from `tests/roms/` for header parsing tests.
  - `parse_header_real_rom`: call `CartridgeHeader::parse` on a real ROM from the bundle (any MBC type). Assert `header.title`, `header.cartridge_type`, `header.rom_size`, `header.ram_size` match the values read from bytes 0x0134/0x0147/0x0148/0x0149 of that file. Use `std::fs::read("tests/roms/<path>").expect("run scripts/download-test-roms.sh")` to load the file.
  - `load_unsupported_mbc_real_rom`: `Cartridge::load` on the same real MBC1+ ROM → `Err` containing `"unsupported MBC"`.
  - `load_too_small`: `Cartridge::load(vec![0u8; 10])` → `Err` containing `"ROM too small"`.
  - `load_flat_rom`: if a flat ROM (MBC 0x00) exists in the bundle use it; otherwise use `minimal_rom(0x00)` (helper: 0x014A-byte vec, `b"TEST"` at 0x0134, `0x00` at 0x0147). Assert `header.cartridge_type == 0x00` and load succeeds.
  - `read_out_of_bounds`: `Cartridge::read(0x7FFF)` on a 0x014A-byte ROM → `0xFF`.

  **bus.rs** — real test ROMs are MBC1+ so can't be loaded into a `Bus` in Phase 1; use `minimal_rom(0x00)` synthetic stub. Helper `fn test_bus() -> Bus` builds a `minimal_rom(0x00)` and wraps it in `Bus::new`:
  - `wram_roundtrip`: write `0x42` to `0xC000`, read back `0x42`
  - `hram_roundtrip`: write `0xAB` to `0xFF80`, read back `0xAB`
  - `echo_ram_mirrors_wram`: write `0x55` to `0xC000`, read `0xE000` returns `0x55`
  - `unmapped_reads_ff`: read `0xFF00` (I/O) and `0xFFFF` (IE) both return `0xFF`
  - `rom_write_silent`: write `0xAA` to `0x0000` does not panic; read `0x0000` returns the original ROM byte

## Manual Testing

1. Run `./preflight.sh` (includes `cargo test`) and confirm it exits 0.
2. Serve with `python -m http.server 8000` and open `http://localhost:8000/www/`.
3. Confirm "gpuboy ready" still appears in the console (no regression).
4. Load a flat ROM (e.g. Tetris) via the file picker. Confirm the cartridge title (e.g. `TETRIS`) appears in the console.
5. Load an MBC ROM (e.g. Pokemon Red). Confirm an error containing "unsupported MBC" is logged.
6. Confirm no unhandled JS exceptions in DevTools → Console.

**Green light:** [ ]
