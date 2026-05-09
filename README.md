# gpuboy

A Game Boy (DMG) emulator that runs in the browser. Built with Rust, compiled to WebAssembly, rendered via WebGPU.

**[Live demo](https://webasmgames.github.io/gpuboy/www/)**

## Features

- LR35902 CPU — passes Blargg `cpu_instrs`, `instr_timing`, `mem_timing`; passes relevant Mooneye timing tests
- PPU — scanline renderer, sprites, scrolling background, window layer
- APU — all 4 channels, ~44 kHz output via `ScriptProcessorNode`
- MBC1, MBC3, MBC5 cartridge support
- WebGPU renderer; falls back to 2D canvas in browsers without WebGPU
- ROM loading from `.gb`, `.gbc`, or `.zip` via the file picker
- Keyboard, gamepad, and on-screen touch controls
- No JS bundler required — wasm-pack `--target web` + a plain HTTP server

## Controls

| Input | Game Boy |
|---|---|
| W A S D | D-pad |
| Arrow Right | A |
| Arrow Left | B |
| Arrow Down | Start |
| Arrow Up | Select |
| Gamepad | Standard mapping |

On-screen D-pad and A/B/Select/Start buttons are also available for touch.

## Running locally

```bash
wasm-pack build crates/gpuboy-wasm --target web --release --out-dir ../../pkg
python -m http.server 8000
# open http://localhost:8000/www/
```

Requires: Rust, [wasm-pack](https://rustwasm.github.io/wasm-pack/), Python 3.

## Development

```bash
# Lint and format
cargo fmt -- --check
cargo clippy -- -D warnings

# Tests (includes Blargg + Mooneye headless)
cargo test

# Full preflight (fmt + clippy + tests + wasm build + esbuild minify)
./preflight.sh
```

`esbuild` must be on PATH for `preflight.sh` (`npm install -g esbuild`).

## Project layout

```
crates/
  gpuboy-core/    # emulator logic (CPU, PPU, APU, MBC)
  gpuboy-wasm/    # wasm-bindgen boundary — stays thin
www/
  index.html      # browser shell
  index.js        # WASM init, ROM loading, input, audio
  style.css       # DMG-aesthetic shell
.github/workflows/
  ci.yml          # fmt + clippy + tests + wasm-pack --release + esbuild + Pages deploy
```

## Architecture notes

- Audio drives the frame loop: `ScriptProcessorNode` fires ~10.7×/sec; each callback steps the emulator by one audio buffer (4096 samples) and hands back an RGBA framebuffer.
- WebGPU path uploads the framebuffer as a 160×144 texture each frame. 2D canvas path uses `putImageData`.
- `ScriptProcessorNode` is deprecated but runs on the main thread where the WASM module lives. `AudioWorklet` would require `SharedArrayBuffer` (needs COOP/COEP headers not available on GitHub Pages).
