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
| 3b | WebGPU Renderer | Replace 2D canvas with WebGPU texture + fullscreen quad | ✅ |
| 4 | Cartridge + MBC Banking | MBC1/3/5, SRAM detection | ✅ |
| [5a](TODO/phase-5a.md) | APU Core | 4 channels, frame sequencer, step_samples | 🔲 |
| [5b](TODO/phase-5b.md) | Audio Clock | ScriptProcessorNode replaces rAF loop | 🔲 |
| 6 | Web UI | ROM file picker, pause/reset, display scaling, touch controls | 🔲 |
| 7 | Joypad Input | Keyboard + gamepad | 🔲 |
| 8 | Save Data | IndexedDB keyed by ROM checksum, .sav export/import | 🔲 |
| 9 | Test ROM Validation | Blargg + Mooneye suites | 🔲 |
| 10 | Game Boy Color | stretch | 🔲 |
| 11 | Stretch Goals | Shaders, rewind, save states, debugger, link cable | 🔲 |

See `TODO/phase-N.md` (or `TODO/phase-Na.md` / `TODO/phase-Nb.md`) for each phase spec.
See `TODO/_template.md` for the spec format.

> - SCRATCH JOSH NOTES
- we build debug not release? check preflight and ci.yml
- some sample roms to load in a dropdown box ? even the testing ones could maybe ref github?
- make a gpuboy background image...
- version tracking?
- changelog? or does git history make a changelog reconstructible for us?
- visible log/console somewhere?
- input should be DPAD/Button images around screen so u can game on mobile
- make a utube/tiktok series on it ?
- node.js 20 is deprecated
- allow loading zips
- clever way to read externals or something idk...
