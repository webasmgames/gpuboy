# gpuboy

A Game Boy (DMG) emulator running in the browser via Rust + WebAssembly + WebGPU.

## Vision

Accurate DMG emulation at 60fps with audio, validated against standard test ROM suites (Blargg, Mooneye). Stretch goals: MBC accuracy, CRT shaders, rewind, audio accuracy.

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
| 5b | Audio Clock | ScriptProcessorNode replaces rAF loop | ✅ |
| 6 | Web UI | Game Boy shell (DMG aesthetic), toolbar icons, play/pause, audio mute, zoom, hamburger menu, touch control stubs | ✅ |
| 6b | Web UI — Sample ROMs | Sample ROM dropdown bundled with the app | ✅ |
| 6c | Visual Polish | Shell sizing with zoom, A/B button slant, error message placement, branding | ✅ |
| 7 | Joypad Input | Keyboard + gamepad + touch D-pad wired up | ✅ |
| 7b | Test ROM Validation | Blargg suite headless in cargo test; Mooneye where feasible | ✅ |
| 8 | Distribution | wasm-pack --release, esbuild minification, ZIP ROM loading via file picker | ✅ |
| [9](TODO/phase-9.md) | Save Data | IndexedDB keyed by ROM checksum, .sav export/import | 🔲 |
| 10 | MBC Accuracy | MBC2 support, MBC3 RTC accuracy + persistence | 🔲 |
| 11 | Shaders | CRT filter, scanlines, LCD grid via WebGPU post-processing pass | 🔲 |
| 12 | Rewind | Frame-by-frame rewind via ring buffer, hold key to step back ~30s | 🔲 |
| 13 | Audio Accuracy | Envelope/sweep timing, length counter edge cases, stereo panning, resampling quality | 🔲 |

See `TODO/phase-N.md` (or `TODO/phase-Na.md` / `TODO/phase-Nb.md`) for each phase spec.
See `TODO/_template.md` for the spec format.

> Changelog: update manually at each green light, or reconstruct from `git log --oneline`.
