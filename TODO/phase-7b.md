# Phase 7b: Test ROM Validation

## Overview

Add a headless test harness that runs real Game Boy test ROMs inside `cargo test`, giving the emulator a regression baseline. Blargg ROMs validate CPU instruction correctness and timing; Mooneye ROMs validate hardware edge cases (timer, PPU, OAM DMA, interrupts, MBC banking). Both suites signal pass/fail via serial output, so the existing `serial_integration` pattern scales directly. All tests are written; those that currently fail are marked `#[ignore]` after Josh's first run — they become the fix queue.

## Requirements

1. WHEN `cargo test --test blargg` runs, THEN each test loads its ROM, steps the emulator until serial output contains `"Passed"` or `"Failed"` (or timeout), and asserts `"Passed"`.
2. WHEN `cargo test --test mooneye` runs, THEN each test loads its ROM, steps the emulator until 6 serial bytes are received (or timeout), and asserts the bytes equal the Fibonacci pass sequence `[3, 5, 8, 13, 21, 34]`.
3. WHEN a ROM file is missing from the expected path, THEN the test panics with the relative ROM path in the error message (not a raw OS error).
4. WHEN a test is known to fail, THEN it is marked `#[ignore]` with a one-line comment naming the unimplemented feature.
5. WHEN `cargo test` runs with no flags, THEN only non-ignored tests run and the suite exits 0; `./preflight.sh` continues to pass.

## Acceptance Criteria

- [ ] `cargo test --test blargg` exits 0 with all non-ignored tests passing.
- [ ] `cargo test --test mooneye` exits 0 with all non-ignored tests passing.
- [ ] `cargo test --test blargg -- --include-ignored` runs all 13 Blargg test functions (some may be ignored).
- [ ] `cargo test --test mooneye -- --include-ignored` runs all 96 Mooneye test functions (some may be ignored).
- [ ] A missing ROM file causes a descriptive panic, not a cryptic IO error.
- [ ] `./preflight.sh` passes (no regressions).

## Design

### Architecture

Two Cargo integration test files alongside the existing unit tests in `gpuboy-core`:

```
crates/gpuboy-core/tests/
  blargg.rs    ← 13 #[test] functions
  mooneye.rs   ← 96 #[test] functions
```

Run selectively:
```bash
cargo test --test blargg
cargo test --test mooneye
cargo test --test blargg -- --include-ignored   # all including ignored
```

Integration tests import the crate as an external user (`use gpuboy_core::Emulator`), so they exercise the public API without special access.

### Key Decisions

**Serial output as the universal signal.** Both suites use the serial port for pass/fail:
- Blargg: prints human-readable text ending in `"Passed"` or `"Failed"`.
- Mooneye: sends exactly 6 bytes — Fibonacci `[3, 5, 8, 13, 21, 34]` for pass, all `0x42` for fail. The same serial-drain loop handles both.

**ROM paths via `CARGO_MANIFEST_DIR`.** Cargo sets this env var to the crate directory (`crates/gpuboy-core`) at compile time. ROM paths are constructed relative to `../../tests/roms/`, which resolves to the workspace-level `tests/roms/` directory. No symlinks, no env var configuration needed.

**`#[ignore]` as the failing-test registry.** Writing all tests up front and ignoring failing ones is more useful than only writing passing tests — the ignore list is a live record of emulator bugs. Each `#[ignore]` comment names the unimplemented feature (e.g., `// boot ROM not implemented`), making the fix queue self-documenting.

**Timeout sizing.** Blargg cpu_instrs subtests take up to ~5 emulated seconds; 600 frames (10 s emulated) is a safe ceiling. Mooneye tests are designed to finish in under 1 emulated second; 120 frames is generous. Mooneye MBC tests need slightly more room: 300 frames. A test that hits the timeout will fail with "timed out" visible in the serial output string or an empty byte slice, making diagnosis straightforward.

**boot_\* tests included but ignored.** The 12 Mooneye `boot_*` tests verify boot ROM register state and timing. Since gpuboy runs without a boot ROM, these will always fail. They are included and pre-marked `#[ignore] // boot ROM not implemented` without needing a test run first.

### Helper design

```rust
// In blargg.rs
fn rom(rel: &str) -> Vec<u8> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read(base.join("../../tests/roms").join(rel))
        .unwrap_or_else(|_| panic!("ROM not found: {rel}"))
}

fn run_blargg(bytes: &[u8], timeout_frames: u32) -> String {
    let mut emu = gpuboy_core::Emulator::new(bytes.to_vec()).expect("load");
    let mut out = String::new();
    for _ in 0..timeout_frames {
        emu.step_frame();
        out.push_str(&String::from_utf8_lossy(&emu.take_serial_output()));
        if out.contains("Passed") || out.contains("Failed") { break; }
    }
    out
}
```

```rust
// In mooneye.rs
fn rom(rel: &str) -> Vec<u8> { /* same as above */ }

fn run_mooneye(bytes: &[u8], timeout_frames: u32) -> Vec<u8> {
    let mut emu = gpuboy_core::Emulator::new(bytes.to_vec()).expect("load");
    let mut serial = Vec::new();
    for _ in 0..timeout_frames {
        emu.step_frame();
        serial.extend(emu.take_serial_output());
        if serial.len() >= 6 { break; }
    }
    serial
}

const PASS: [u8; 6] = [3, 5, 8, 13, 21, 34];
```

### ROM list

**`blargg.rs` — 13 tests** (timeout 600 frames each, except halt_bug/instr_timing at 300):

| Test fn | ROM path |
|---|---|
| `cpu_instrs_01_special` | `blargg/cpu_instrs/individual/01-special.gb` |
| `cpu_instrs_02_interrupts` | `blargg/cpu_instrs/individual/02-interrupts.gb` |
| `cpu_instrs_03_op_sp_hl` | `blargg/cpu_instrs/individual/03-op sp,hl.gb` |
| `cpu_instrs_04_op_r_imm` | `blargg/cpu_instrs/individual/04-op r,imm.gb` |
| `cpu_instrs_05_op_rp` | `blargg/cpu_instrs/individual/05-op rp.gb` |
| `cpu_instrs_06_ld_r_r` | `blargg/cpu_instrs/individual/06-ld r,r.gb` |
| `cpu_instrs_07_jr_jp_call_ret_rst` | `blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb` |
| `cpu_instrs_08_misc` | `blargg/cpu_instrs/individual/08-misc instrs.gb` |
| `cpu_instrs_09_op_r_r` | `blargg/cpu_instrs/individual/09-op r,r.gb` |
| `cpu_instrs_10_bit_ops` | `blargg/cpu_instrs/individual/10-bit ops.gb` |
| `cpu_instrs_11_op_a_hl` | `blargg/cpu_instrs/individual/11-op a,(hl).gb` |
| `halt_bug` | `blargg/halt_bug.gb` |
| `instr_timing` | `blargg/instr_timing/instr_timing.gb` |

**`mooneye.rs` — 96 tests** organized by path category. Pre-mark the 12 `boot_*` tests with `#[ignore] // boot ROM not implemented`. All others run and get `#[ignore]` added by Josh after the first test run if they fail.

| Category | Path prefix | Count |
|---|---|---|
| Top-level timing / control flow | `mooneye-test-suite/acceptance/` | 41 |
| Bits | `mooneye-test-suite/acceptance/bits/` | 3 |
| Instruction | `mooneye-test-suite/acceptance/instr/` | 1 |
| Interrupts | `mooneye-test-suite/acceptance/interrupts/` | 1 |
| OAM DMA | `mooneye-test-suite/acceptance/oam_dma/` | 3 |
| PPU | `mooneye-test-suite/acceptance/ppu/` | 12 |
| Serial | `mooneye-test-suite/acceptance/serial/` | 1 |
| Timer | `mooneye-test-suite/acceptance/timer/` | 13 |
| MBC1 | `mooneye-test-suite/emulator-only/mbc1/` | 13 |
| MBC5 | `mooneye-test-suite/emulator-only/mbc5/` | 8 |

Function naming convention: replace non-alphanumeric chars with `_`, collapse consecutive underscores, strip leading/trailing underscores. E.g. `boot_div2-S.gb` → `fn boot_div2_s()`.

Timeout: 120 frames for acceptance tests; 300 frames for mbc1/mbc5 tests.

## Tasks

- [ ] 1. Create `crates/gpuboy-core/tests/blargg.rs` with `rom()` helper, `run_blargg()` helper, and 13 `#[test]` functions as listed in the ROM table above. *(req 1, 3)*

- [ ] 2. Create `crates/gpuboy-core/tests/mooneye.rs` with `rom()` helper, `run_mooneye()` helper, `PASS` constant, and 96 `#[test]` functions. Pre-mark all 12 `boot_*` tests with `#[ignore] // boot ROM not implemented`. *(req 2, 3, 4)*

- [ ] 3. Josh runs `cargo test --test blargg -- --include-ignored` and `cargo test --test mooneye -- --include-ignored`; adds `#[ignore]` with a descriptive comment to every test that fails. *(req 4)*

- [ ] 4. Confirm `cargo test` (no flags) exits 0. Run `./preflight.sh` and confirm PASS. *(req 5)*

## Manual Testing

1. Run `cargo test --test blargg -- --include-ignored 2>&1 | tail -20`. Note which tests pass and which fail.
2. Run `cargo test --test mooneye -- --include-ignored 2>&1 | tail -20`. Note which tests pass and which fail.
3. Add `#[ignore]` with comments to all failing tests in both files.
4. Run `cargo test` (no flags). Confirm exit 0 and all listed tests pass.
5. Run `./preflight.sh`. Confirm PASS.
6. Spot-check: run `cargo test --test blargg cpu_instrs_06` (a likely-passing test). Confirm it passes individually.

**Green light:** [ ]
