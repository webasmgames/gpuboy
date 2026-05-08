# Phase 4: Cartridge + MBC Banking

## Overview

Replaces the flat, MBC0-only `Cartridge` struct with a proper cartridge abstraction that supports MBC1, MBC3, and MBC5 in addition to ROM-only (MBC0). Games like Pokémon Red (MBC1), Pokémon Crystal (MBC3), and Pokémon Gold (MBC5) all require bank switching to access ROMs larger than 32 KiB. Without banking, every commercial Game Boy title that uses an MBC type crashes or produces garbage output. This phase adds SRAM (in-memory only, no persistence) and makes the emulator capable of running the vast majority of the Game Boy library.

## Requirements

1. WHEN a ROM with cartridge type byte 0x00 is loaded, THEN the emulator treats it as ROM-only (MBC0) and behavior is identical to prior phases.

2. WHEN a ROM with cartridge type byte 0x01, 0x02, or 0x03 is loaded, THEN the emulator uses MBC1 banking logic for all reads and writes in the 0x0000–0x7FFF and 0xA000–0xBFFF address ranges.

3. WHEN a ROM with cartridge type byte 0x0F, 0x10, 0x11, 0x12, or 0x13 is loaded, THEN the emulator uses MBC3 banking logic. RTC register access (RAM bank values 0x08–0x0C) is silently ignored.

4. WHEN a ROM with cartridge type byte 0x19, 0x1A, 0x1B, 0x1C, 0x1D, or 0x1E is loaded, THEN the emulator uses MBC5 banking logic.

5. WHEN a ROM with an unrecognized cartridge type byte is loaded, THEN `Cartridge::load` returns an `Err` describing the unsupported MBC type, and `load_rom` in the WASM layer logs the error to the console without panicking.

6. WHEN the ROM header at byte 0x0149 indicates RAM is present (values 0x02–0x05), THEN the emulator allocates an in-memory SRAM buffer sized according to the table in the Design section. No persistence to disk or browser storage is performed.

7. WHEN an MBC-enabled ROM reads from 0xA000–0xBFFF and SRAM is enabled and allocated, THEN the correct SRAM bank byte is returned.

8. WHEN an MBC-enabled ROM writes to 0xA000–0xBFFF and SRAM is enabled and allocated, THEN the correct SRAM bank byte is updated.

9. WHEN an MBC-enabled ROM reads from 0xA000–0xBFFF and SRAM is not enabled or not present, THEN 0xFF is returned.

10. WHEN an MBC-enabled ROM writes to 0xA000–0xBFFF and SRAM is not enabled or not present, THEN the write is silently discarded.

## Acceptance Criteria

- [ ] Loading a flat ROM (MBC0, e.g. Tetris) works as before: the emulator initializes without error and renders frames.
- [ ] Loading a Pokémon Red ROM (MBC1, cartridge type 0x13) initializes without error and renders the Nintendo logo + title screen.
- [ ] Loading a Super Mario Land 2 ROM (MBC1, cartridge type 0x1B — note: actually MBC5; use a confirmed MBC1 title like Dr. Mario 0x01 or Kirby's Dream Land 0x01 if SML2 is MBC5) — see note in Design §Key Decisions on confirmed titles.
- [ ] Loading a Pokémon Crystal ROM (MBC3, cartridge type 0x10) initializes without error and renders the GBC splash / opening frames without a panic.
- [ ] Loading a Pokémon Gold ROM (MBC5, cartridge type 0x1B) initializes without error and renders opening frames without a panic.
- [ ] A ROM with an unknown MBC type (e.g. a header stub with cartridge type 0xFF) logs an error to the browser console and does not crash (no panic, no blank white page).
- [ ] The existing `serial_integration` unit test still passes (`cargo test -p gpuboy-core`).
- [ ] The existing `bus` unit tests still pass.
- [ ] The existing cartridge unit tests that were previously passing still pass (the `load_flat_rom`, `read_out_of_bounds`, `load_too_small` tests).
- [ ] The `parse_header_real_rom` and `load_unsupported_mbc_real_rom` tests in the old cartridge test suite are updated: `load_unsupported_mbc_real_rom` should now expect `Ok` (since cpu_instrs.gb is MBC1, cartridge type 0x01).

## Design

### Architecture

The existing `Cartridge` struct and `CartridgeHeader` struct in `crates/gpuboy-core/src/cartridge.rs` are replaced entirely. The new design introduces a `Mbc` enum with a variant per MBC type. `Cartridge` becomes a thin wrapper that owns the ROM bytes, SRAM bytes, and a `Mbc` variant. `Bus` already holds a `Cartridge` and delegates 0x0000–0x7FFF reads and 0xA000–0xBFFF reads/writes to it — that dispatch stays in `bus.rs`, but the `Bus::write` match arm for 0x0000–0x7FFF must be updated from a silent no-op to actually forwarding the write to `self.cartridge.write(addr, val)` so MBC register writes reach the cartridge.

```
crates/gpuboy-core/src/
  cartridge.rs   — CartridgeHeader (unchanged parse logic), Mbc enum, Cartridge struct
  bus.rs         — Bus::write 0x0000..=0x7FFF arm: call self.cartridge.write(addr, val)
                   Bus::read 0xA000..=0xBFFF: call self.cartridge.read(addr)
                   Bus::write 0xA000..=0xBFFF: call self.cartridge.write(addr, val)
```

No other crates change.

### Data Structures

#### ROM size table (byte 0x0148)

| Header value | ROM size  | Number of 16 KiB banks |
|---|---|---|
| 0x00 | 32 KiB | 2 |
| 0x01 | 64 KiB | 4 |
| 0x02 | 128 KiB | 8 |
| 0x03 | 256 KiB | 16 |
| 0x04 | 512 KiB | 32 |
| 0x05 | 1 MiB | 64 |
| 0x06 | 2 MiB | 128 |
| 0x07 | 4 MiB | 256 |
| 0x08 | 8 MiB | 512 |

The number of banks is `2 << rom_size_byte` for values 0x00–0x08. This value is used to mask the ROM bank number and validate ROM data length.

#### RAM size table (byte 0x0149)

| Header value | RAM size | Number of 8 KiB banks |
|---|---|---|
| 0x00 | None | 0 |
| 0x01 | Unused / 2 KiB (treat as 0) | 0 |
| 0x02 | 8 KiB | 1 |
| 0x03 | 32 KiB | 4 |
| 0x04 | 128 KiB | 16 |
| 0x05 | 64 KiB | 8 |

Total SRAM bytes = number of banks × 8192. Values 0x00 and 0x01 allocate no SRAM (`Vec::new()`). Values 0x02–0x05 allocate `num_ram_banks * 8192` zero-filled bytes.

#### `Mbc` enum

```rust
#[derive(Debug)]
pub enum Mbc {
    None,
    Mbc1 {
        rom_bank: u8,       // 5-bit lower ROM bank register (reset value: 1)
        ram_bank: u8,       // 2-bit RAM bank / upper ROM bank register (reset value: 0)
        mode: bool,         // false = ROM banking mode, true = RAM banking mode
        ram_enabled: bool,  // true when 0x0A written to 0x0000–0x1FFF
    },
    Mbc3 {
        rom_bank: u8,       // 7-bit ROM bank register (reset value: 1)
        ram_bank: u8,       // 2-bit RAM bank register (reset value: 0)
        ram_enabled: bool,  // true when 0x0A written to 0x0000–0x1FFF
    },
    Mbc5 {
        rom_bank_lo: u8,    // lower 8 bits of ROM bank (reset value: 1)
        rom_bank_hi: u8,    // bit 0 = upper 1 bit of ROM bank (reset value: 0)
        ram_bank: u8,       // 4-bit RAM bank register (reset value: 0)
        ram_enabled: bool,  // true when 0x0A written to 0x0000–0x1FFF
    },
}
```

#### `Cartridge` struct

```rust
#[derive(Debug)]
pub struct Cartridge {
    pub header: CartridgeHeader,
    rom: Vec<u8>,
    sram: Vec<u8>,       // empty if no RAM present
    mbc: Mbc,
}
```

`CartridgeHeader` retains its existing fields and `parse()` method unchanged.

### Cartridge load and MBC dispatch

`Cartridge::load(rom: Vec<u8>) -> Result<Self, String>` must:

1. Call `CartridgeHeader::parse(&rom)?`.
2. Derive `num_rom_banks` from `header.rom_size` using the table above. If `rom.len() < num_rom_banks * 16384`, return `Err("ROM data shorter than header claims".into())`. For ROMs where `rom.len()` exceeds what the header claims, accept silently (truncation-safe; indexing is bounds-checked via `.get().copied().unwrap_or(0xFF)`).
3. Allocate `sram` from the RAM size table.
4. Construct the `Mbc` variant based on `header.cartridge_type`:
   - `0x00` → `Mbc::None`
   - `0x01 | 0x02 | 0x03` → `Mbc::Mbc1 { rom_bank: 1, ram_bank: 0, mode: false, ram_enabled: false }`
   - `0x0F | 0x10 | 0x11 | 0x12 | 0x13` → `Mbc::Mbc3 { rom_bank: 1, ram_bank: 0, ram_enabled: false }`
   - `0x19 | 0x1A | 0x1B | 0x1C | 0x1D | 0x1E` → `Mbc::Mbc5 { rom_bank_lo: 1, rom_bank_hi: 0, ram_bank: 0, ram_enabled: false }`
   - anything else → `return Err(format!("unsupported MBC: 0x{:02X}", header.cartridge_type))`
5. Return `Ok(Cartridge { header, rom, sram, mbc })`.

### `Cartridge::read(addr: u16) -> u8`

Match on `addr`:

**0x0000–0x3FFF (ROM bank 0 area):**

For `Mbc::None` and `Mbc::Mbc5`: always reads from physical address `addr as usize`.

For `Mbc::Mbc1` in RAM banking mode (`mode == true`): the upper 2-bit `ram_bank` field selects the 1 MiB block. Physical address = `(ram_bank as usize) << 19 | addr as usize`. Mask the result with the ROM size mask (see §MBC1 bank masking below).

For `Mbc::Mbc1` in ROM banking mode (`mode == false`) and for `Mbc::Mbc3`: always reads from `addr as usize` (fixed bank 0).

**0x4000–0x7FFF (ROM bank N area):**

Physical address = `bank_number as usize * 0x4000 + (addr as usize - 0x4000)`.

Effective bank numbers per MBC type:

- `Mbc::None`: bank 1 (fixed). Physical = `0x4000 + (addr - 0x4000) as usize`.
- `Mbc::Mbc1`: see §MBC1 bank masking. Effective bank = `((ram_bank as usize) << 5 | (rom_bank as usize)) & rom_mask`. If the masked result is 0 or a "bank 0 alias" (see below), it becomes 1.
- `Mbc::Mbc3`: effective bank = `rom_bank as usize` (already 1–127; the write logic ensures it is never 0 — if the game writes 0x00 to the register, treat as 0x01). Physical = `rom_bank as usize * 0x4000 + (addr - 0x4000) as usize`.
- `Mbc::Mbc5`: effective bank = `(rom_bank_hi as usize) << 8 | rom_bank_lo as usize`. This is a 9-bit value (0–511). Unlike MBC1/MBC3, bank 0 is addressable in the 0x4000–0x7FFF window on MBC5 (no auto-correct to 1). Physical = `bank * 0x4000 + (addr - 0x4000) as usize`.

All physical addresses are bounds-checked: use `self.rom.get(phys).copied().unwrap_or(0xFF)`.

**0xA000–0xBFFF (SRAM):**

If `sram.is_empty()` or `ram_enabled == false`: return `0xFF`.

Effective RAM bank:
- `Mbc::Mbc1` in RAM banking mode: `ram_bank & 0x03`.
- `Mbc::Mbc1` in ROM banking mode: 0.
- `Mbc::Mbc3`: `ram_bank & 0x03` (values 0x08–0x0C are RTC stubs; if `ram_bank >= 0x08` return `0xFF`).
- `Mbc::Mbc5`: `ram_bank & 0x0F`.

Physical SRAM address = `ram_bank_eff as usize * 0x2000 + (addr - 0xA000) as usize`.

Bounds-check: use `self.sram.get(phys).copied().unwrap_or(0xFF)`.

### `Cartridge::write(addr: u16, val: u8)`

**For `Mbc::None`: all writes are silently discarded.**

#### MBC1 write dispatch

| Address range | Effect |
|---|---|
| 0x0000–0x1FFF | If `val & 0x0F == 0x0A`: set `ram_enabled = true`. Otherwise: set `ram_enabled = false`. |
| 0x2000–0x3FFF | Set `rom_bank = val & 0x1F`. If result is 0, set to 1 (bank 0 alias). |
| 0x4000–0x5FFF | Set `ram_bank = val & 0x03`. |
| 0x6000–0x7FFF | Set `mode = val & 0x01 != 0`. |
| 0xA000–0xBFFF | If `ram_enabled` and `sram` is not empty: compute `ram_bank_eff` as in read (mode-dependent), write `self.sram[phys] = val`. Otherwise discard. |

#### MBC1 bank masking

Let `num_rom_banks = 2usize << header.rom_size`. The ROM mask = `num_rom_banks - 1`.

When reading from 0x4000–0x7FFF:
- Full bank bits = `(ram_bank as usize) << 5 | (rom_bank as usize)`
- Apply mask: `effective = full_bank & (num_rom_banks - 1)`
- If `effective == 0`: set `effective = 1` (bank 0 alias; the hardware quirk prevents accessing bank 0 at 0x4000–0x7FFF)

When reading from 0x0000–0x3FFF in RAM banking mode:
- Full bank bits = `(ram_bank as usize) << 5`
- Apply mask: `effective = full_bank & (num_rom_banks - 1)`
- Physical = `effective * 0x4000 + addr as usize`

#### MBC3 write dispatch

| Address range | Effect |
|---|---|
| 0x0000–0x1FFF | If `val & 0x0F == 0x0A`: `ram_enabled = true`. Otherwise `ram_enabled = false`. |
| 0x2000–0x3FFF | Set `rom_bank = val & 0x7F`. If result is 0, set to 1. |
| 0x4000–0x5FFF | If `val <= 0x03`: set `ram_bank = val`. If `val >= 0x08 && val <= 0x0C`: set `ram_bank = val` (RTC select stub — reads/writes to SRAM with `ram_bank >= 0x08` return 0xFF / discard, handled in read/write). |
| 0x6000–0x7FFF | Latch RTC — silently ignored (no RTC this phase). |
| 0xA000–0xBFFF | If `ram_enabled` and `sram` not empty and `ram_bank <= 0x03`: write `self.sram[phys] = val`. Otherwise discard. |

#### MBC5 write dispatch

| Address range | Effect |
|---|---|
| 0x0000–0x1FFF | If `val & 0x0F == 0x0A`: `ram_enabled = true`. Otherwise `ram_enabled = false`. |
| 0x2000–0x2FFF | Set `rom_bank_lo = val` (all 8 bits). |
| 0x3000–0x3FFF | Set `rom_bank_hi = val & 0x01` (only bit 0 is used). |
| 0x4000–0x5FFF | Set `ram_bank = val & 0x0F`. |
| 0xA000–0xBFFF | If `ram_enabled` and `sram` not empty: write `self.sram[phys] = val`. Otherwise discard. |

### `Bus` changes

Two changes to `bus.rs`:

1. **ROM write arm**: change `0x0000..=0x7FFF => {}` to `0x0000..=0x7FFF => self.cartridge.write(addr, val)`. This is the only change required for MBC register writes to work.

2. **SRAM read/write arms**: add explicit match arms for 0xA000–0xBFFF:
   - In `Bus::read`: `0xA000..=0xBFFF => self.cartridge.read(addr)`
   - In `Bus::write`: `0xA000..=0xBFFF => self.cartridge.write(addr, val)`

Currently the bus falls through to the `_ => 0xFF` / `_ => {}` catch-all for 0xA000–0xBFFF. The new explicit arms must be placed before the catch-all.

### Key Decisions

**`Mbc` as an enum, not a trait object.** A trait object (`Box<dyn Mbc>`) would require `dyn` dispatch and boxing. An enum keeps all banking state inline, avoids heap allocation per cartridge, and lets the compiler monomorphize each `match` arm. The number of MBC variants is small and known at compile time.

**No RTC for MBC3.** Implementing RTC requires tracking real wall-clock time and an RTC latch/register file. Games that use RTC (Pokémon Gold/Silver/Crystal) boot and run correctly without it — they read back stub 0xFF values and degrade gracefully (e.g., in-game clock shows wrong time). This is deferred to Phase 8 along with save-file persistence.

**SRAM is in-memory only.** Browser `localStorage` / OPFS persistence is Phase 8. Allocating SRAM now is necessary because games write to it unconditionally during boot (e.g., Pokémon initializes save data); without an allocated buffer, writes would be silently dropped, reads return 0xFF, and the game may behave unexpectedly or crash. An empty `Vec` for no-RAM carts vs. a sized zero-filled `Vec` for RAM carts is cheap and correct.

**ROM size validation.** If the ROM file is shorter than the header claims (e.g., a truncated download), the emulator returns an error rather than silently returning 0xFF from out-of-bounds reads. This surfaces corrupt ROMs early. ROMs longer than claimed (e.g., homebrew with padding) are accepted.

**MBC5 bank 0 in 0x4000–0x7FFF window.** Unlike MBC1 and MBC3, MBC5 does not have the bank-0 alias quirk. Writing 0x00 to the ROM bank register is valid and selects bank 0. The auto-correct-to-1 logic used in MBC1/MBC3 must NOT be applied to MBC5.

**Confirmed test titles and cartridge types:**
- Tetris (DMG, 1989): cartridge type 0x00 (ROM only, MBC0)
- Pokémon Red: cartridge type 0x13 (MBC3+RAM+BATTERY — actually MBC1 on NA/EU cartridges; the header for the most common ROM dump is 0x13 which maps to MBC3 in this spec. Use Pokémon Blue (0x13 MBC3) or check header byte 0x0147 of your specific dump before testing.)
- Dr. Mario: cartridge type 0x01 (MBC1, no RAM)
- Kirby's Dream Land: cartridge type 0x01 (MBC1, no RAM)
- Super Mario Land 2: cartridge type 0x1B (MBC5+RAM+BATTERY)
- Pokémon Crystal: cartridge type 0x10 (MBC3+TIMER+RAM+BATTERY)

Verify `xxd <rom.gb> | head -5` and check offset 0x0147 before testing acceptance criteria with specific ROMs. The criteria above use MBC1 titles confirmed to be 0x01/0x02/0x03 (Dr. Mario, Kirby's Dream Land) plus an MBC3 title (Pokémon Crystal 0x10) and an MBC5 title (Super Mario Land 2 0x1B).

## Tasks

- [x] 1. In `crates/gpuboy-core/src/cartridge.rs`, replace the `Cartridge` struct and `Cartridge::load`/`Cartridge::read` impl with the new design. Keep `CartridgeHeader` and its `parse()` method exactly as-is (same fields, same byte offsets: title 0x0134–0x0143, cartridge_type 0x0147, rom_size 0x0148, ram_size 0x0149, minimum length 0x014A). *(req 1–5)*

   Specifically:
   - Add the `Mbc` enum (with `None`, `Mbc1`, `Mbc3`, `Mbc5` variants) exactly as shown in §Data Structures.
   - Add `sram: Vec<u8>` and `mbc: Mbc` fields to the `Cartridge` struct alongside existing `header` and `rom` fields.
   - Implement `Cartridge::load` per §Cartridge load and MBC dispatch: parse header, derive `num_rom_banks` (`2usize << header.rom_size`), validate ROM length, allocate `sram` from RAM size table, construct `Mbc` variant from `header.cartridge_type`, return error for unsupported types.

   RAM size allocation helper (put as a free function or inside `load`):
   ```rust
   fn sram_size(ram_size_byte: u8) -> usize {
       match ram_size_byte {
           0x02 => 8192,
           0x03 => 32768,
           0x04 => 131072,
           0x05 => 65536,
           _ => 0,
       }
   }
   ```
   *(req 1–5, 6)*

- [x] 2. Implement `Cartridge::read(&self, addr: u16) -> u8` per §`Cartridge::read`. Handle all three address regions in a single `match addr`:

   - `0x0000..=0x3FFF`: MBC1 RAM-banking-mode uses upper bits of `ram_bank`; all others use fixed bank 0.
   - `0x4000..=0x7FFF`: dispatch per MBC type. Apply MBC1 bank masking. MBC3 zero-to-one correction. MBC5 no zero correction.
   - `0xA000..=0xBFFF`: check `ram_enabled` and `sram` non-empty; compute physical SRAM address from bank; bounds-check with `.get().copied().unwrap_or(0xFF)`.
   - Any other address: return `0xFF`.

   All ROM accesses must use `self.rom.get(phys).copied().unwrap_or(0xFF)`.
   *(req 1–4, 7, 9)*

- [x] 3. Implement `Cartridge::write(&mut self, addr: u16, val: u8)` per §`Cartridge::write`. Handle all write address ranges for each MBC type. For `Mbc::None`, all writes are no-ops. For MBC1/MBC3/MBC5, dispatch register writes and SRAM writes exactly as in the tables in §`Cartridge::write`. *(req 1–4, 8, 10)*

- [x] 4. Update `crates/gpuboy-core/src/bus.rs`:

   a. In `Bus::write`, change the `0x0000..=0x7FFF` arm from `{}` (no-op) to `self.cartridge.write(addr, val)`. *(req 2–4)*

   b. In `Bus::read`, add the arm `0xA000..=0xBFFF => self.cartridge.read(addr)` before the catch-all `_ => 0xFF`. *(req 7, 9)*

   c. In `Bus::write`, add the arm `0xA000..=0xBFFF => self.cartridge.write(addr, val)` before the catch-all `_ => {}`. *(req 8, 10)*

   No other changes to `bus.rs` are needed.

- [x] 5. Update the unit tests in `crates/gpuboy-core/src/cartridge.rs`:

   a. The `load_unsupported_mbc_real_rom` test currently asserts `Cartridge::load` returns an error for `cpu_instrs.gb` (MBC1, type 0x01). Change this test to assert `Cartridge::load` returns `Ok` (since MBC1 is now supported). Verify `cart.header.cartridge_type == 0x01`.

   b. Keep `parse_header_real_rom`, `load_too_small`, `load_flat_rom`, and `read_out_of_bounds` tests unchanged — they should still pass.

   c. Add a unit test `mbc1_rom_bank_select` that constructs a 128 KiB ROM (8 banks × 16 KiB), sets distinct sentinel bytes at the start of each bank, and verifies reads from 0x4000–0x7FFF return the correct sentinel after writing each bank number 1–7 to address 0x2000. *(req 2)*

   d. Add a unit test `mbc1_ram_bank_select` that constructs a minimal MBC1+RAM cart (header type 0x03, ram_size 0x02 = 8 KiB), enables SRAM (write 0x0A to 0x0000), writes a value via 0xA000, and reads it back. *(req 8)*

   e. Add a unit test `mbc3_rom_bank_zero_becomes_one` that writes 0x00 to address 0x2000 on an MBC3 cartridge and confirms reads from 0x4000 still return data from bank 1 (not bank 0). *(req 3)*

   f. Add a unit test `mbc5_bank_zero_accessible` that writes 0x00 to address 0x2000 on an MBC5 cartridge and confirms reads from 0x4000 return bank 0 data (not bank 1). *(req 4)*

   g. Add a unit test `sram_disabled_reads_ff` that confirms reading from 0xA000 without enabling SRAM returns 0xFF for all MBC types that support SRAM. *(req 9)*

   h. Add a unit test `unknown_mbc_returns_err` that creates a minimal ROM with cartridge type 0xFF and asserts `Cartridge::load` returns an `Err` containing "unsupported MBC". *(req 5)*

- [x] 6. Verify no changes are needed in `crates/gpuboy-wasm/src/lib.rs`. The `load_rom` function already logs `Err` strings to the console via `web_sys::console::log_1`, which satisfies req 5 for the browser path. No changes needed. *(req 5)*

## Manual Testing

1. Run `cargo test -p gpuboy-core` and confirm all tests pass, including the new MBC tests and the updated `load_unsupported_mbc_real_rom` test.

2. Build: `wasm-pack build crates/gpuboy-wasm --target web`. Confirm it completes without errors.

3. Serve: `python -m http.server 8000`, open `http://localhost:8000/www/` in Chrome/Edge. Open DevTools console.

4. Load Tetris (MBC0, cartridge type 0x00). Confirm the emulator initializes, the game title logs to console, and the Nintendo logo / title screen renders.

5. Load Dr. Mario or Kirby's Dream Land (MBC1, cartridge type 0x01). Confirm the title logs, no error appears in console, and the Nintendo logo / title screen renders without visual corruption.

6. Load a Pokémon Red or Blue ROM (verify `xxd pokemon_red.gb | grep -m1 ''` and check byte at offset 0x147). Confirm initialization, no crash, and title screen renders.

7. Load a Pokémon Crystal ROM (MBC3, cartridge type 0x10). Confirm initialization, no crash, and the GBC boot splash / opening animation renders.

8. Load Super Mario Land 2 (MBC5, cartridge type 0x1B). Confirm initialization, no crash, and the Nintendo logo / title screen renders.

9. Test error path: create a minimal stub ROM (any hex editor, or `python3 -c "import sys; d=bytearray(0x150); d[0x0147]=0xFF; sys.stdout.buffer.write(d)"  > bad.gb`) and load it. Confirm the console logs an "unsupported MBC: 0xFF" error and the page does not crash or display a blank error screen.

10. Confirm existing Phase 3b functionality is unaffected: wgpu renderer toggles correctly, pixels are sharp, no regressions in flat-ROM display.

**Green light:** [x]
