# gpuboy

A Game Boy (DMG) emulator running in the browser via Rust + WebAssembly + WebGPU.

## Vision

Accurate DMG emulation at 60fps with audio, validated against standard test ROM suites (Blargg, Mooneye). Game Boy Color (GBC) as a stretch goal.

## Phases

| Phase | Name | Notes | Status |
|---|---|---|---|
| 0 | Project Scaffold | Rust + wasm-pack + browser shell + CI + GitHub Pages | ✅ |
| 1 | Memory Bus + ROM Loading | Flat ROM only; no MBC yet | ✅ |
| 2 | CPU + Timer + Interrupts + Serial Stub | LR35902, DIV/TIMA/TMA/TAC, IME, serial while CPU is hot | ✅ |
| 3a | PPU Core | Scanline renderer, RGBA framebuffer, 2D canvas display | ✅ |
| [3b](TODO/phase-3b.md) | WebGPU Renderer | Replace 2D canvas with WebGPU texture + fullscreen quad | 🔲 |
| 4 | Web UI | ROM file picker, pause/reset, display scaling | 🔲 |
| 5 | Joypad Input | Keyboard + gamepad | 🔲 |
| 6 | Cartridge + MBC Banking | MBC1/3/5, SRAM detection | 🔲 |
| 7 | Save Data | IndexedDB keyed by ROM checksum, .sav export/import | 🔲 |
| 8 | APU + Web Audio | 4 channels, Web Audio as master clock | 🔲 |
| 9 | Test ROM Validation | Blargg + Mooneye suites | 🔲 |
| 10 | Game Boy Color | stretch | 🔲 |
| 11 | Stretch Goals | Shaders, rewind, save states, debugger, link cable | 🔲 |

See `TODO/phase-N.md` (or `TODO/phase-Na.md` / `TODO/phase-Nb.md`) for each phase spec.
See `TODO/_template.md` for the spec format.

> - SCRATCH JOSH NOTES
- some sample roms to load in a dropdown box ? even the testing ones could maybe ref github?
- make a gpuboy background image...
- version tracking?
- changelog? or does git history make a changelog reconstructible for us?
- visible log/console somewhere?
- make a utube/tiktok series on it ?