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
| 5a | APU Core | 4 channels, frame sequencer, step_samples | ✅ |
| [5b](TODO/phase-5b.md) | Audio Clock | ScriptProcessorNode replaces rAF loop | 🔲 |
| 6 | Web UI | ROM file picker, sample ROM dropdown, pause/reset, display scaling, touch controls, log overlay, background image | 🔲 |
| 7 | Joypad Input | Keyboard + gamepad + touch D-pad wired up | 🔲 |
| 8 | Save Data | IndexedDB keyed by ROM checksum, .sav export/import | 🔲 |
| 8b | Distribution | Release builds in preflight + CI, ZIP ROM loading, URL ROM loading (?rom=), Node.js version bump | 🔲 |
| 9 | Test ROM Validation | Blargg + Mooneye suites | 🔲 |
| 10 | Game Boy Color | stretch | 🔲 |
| 11 | Stretch Goals | Shaders, rewind, save states, debugger, link cable, YouTube/TikTok-ready demo | 🔲 |

See `TODO/phase-N.md` (or `TODO/phase-Na.md` / `TODO/phase-Nb.md`) for each phase spec.
See `TODO/_template.md` for the spec format.

> Changelog: update manually at each green light, or reconstruct from `git log --oneline`.
