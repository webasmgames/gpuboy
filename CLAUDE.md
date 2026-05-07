# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**gpuboy** is a Game Boy (DMG) emulator targeting the browser via Rust + WebAssembly + WebGPU. See `TODO.md` for the phase roadmap and `CONSTITUTION.md` for the working agreement.

## Rules (from CONSTITUTION.md)

**Claude can:**
- Read and edit source files, create new source files
- Research documentation and reference material
- Run `cargo fmt` and `cargo clippy` — on demand only, when Josh explicitly asks

**Claude cannot:**
- Run any git command except read-only inspection: `git diff`, `git diff --staged`, `git status`
- Run builds, tests, or compilation: `cargo build`, `cargo test`, `wasm-pack build`, `npm run`, `make`, or any equivalent
- Run servers, watchers, or any long-lived process

Josh handles all of the above. After Claude edits code, Josh reviews the diff, runs what's needed to verify, then stages, commits, and pushes.

## Phase Workflow

Phases follow a **spec-locked, one-shot** model:

1. Josh and Claude write the phase spec together in `TODO/phase-N.md`
2. Josh confirms the spec is locked
3. In a **fresh Claude session**, Josh says: *"Read TODO/phase-N.md and implement it"*
4. Claude implements the full phase without back-and-forth
5. Josh manually tests per the **Manual Testing** section of the spec and gives the green light

No implementation begins until the spec is locked. No new phase begins until the current phase has a green light.

## Commands Josh Runs

```bash
# Lint and format checks
cargo fmt -- --check
cargo clippy

# Build WASM
wasm-pack build crates/gpuboy-wasm --target web

# Preflight (all checks in sequence)
./preflight.sh

# Serve locally (no bundler needed)
python -m http.server 8000   # then open http://localhost:8000/www/
```

## Architecture

Cargo workspace at repo root. Crates live under `crates/`, browser shell under `www/`, CI under `.github/workflows/`.

```
gpuboy/
  Cargo.toml              # workspace manifest + shared clippy lints
  crates/
    gpuboy-wasm/          # WASM boundary (cdylib) — stays thin
    gpuboy-core/          # emulator logic (Phase 1+)
  www/
    index.html            # browser shell, <canvas id="screen"> 160×144
    index.js              # async WASM init, error display in DOM
  .github/workflows/
    ci.yml                # fmt + clippy + wasm-pack build + GitHub Pages deploy
  pkg/                    # wasm-pack output, gitignored
```

Key decisions:
- `--target web` (not `--target bundler`): no JS build step required; Josh can open `index.html` via a local HTTP server
- `console_error_panic_hook` included from the start so Rust panics surface as readable browser console messages
- Shared lints via `[workspace.lints.clippy]` in root `Cargo.toml`; each crate opts in with `[lints] workspace = true`

## Spec Format

New phase specs (`TODO/phase-N.md`) follow `TODO/_template.md`. Requirements use EARS format (WHEN/THEN). Tasks are numbered, atomic, and reference the requirement(s) they fulfill. Written so Claude can execute the full task list in one session without asking questions.
