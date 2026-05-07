# gpuboy

A Game Boy (DMG) emulator running in the browser via Rust + WebAssembly + WebGPU.

## Vision

Accurate DMG emulation at 60fps with audio, validated against standard test ROM suites (Blargg, Mooneye). Game Boy Color (GBC) as a stretch goal.

## Phases

| Phase | Name | Status |
|---|---|---|
| 0 | [Project Scaffold](TODO/phase-0.md) | 🔲 |
| 1 | Memory Bus | 🔲 |
| 2 | CPU — LR35902 | 🔲 |
| 3 | PPU + WebGPU Renderer | 🔲 |
| 4 | Joypad Input | 🔲 |
| 5 | Cartridge + MBC Banking | 🔲 |
| 6 | APU + Web Audio | 🔲 |
| 7 | Test ROM Validation | 🔲 |
| 8 | Game Boy Color (stretch) | 🔲 |

See `TODO/phase-N.md` for each phase spec.
See `TODO/_template.md` for the spec format.
