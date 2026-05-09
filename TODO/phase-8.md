# Phase 8: Distribution

## Overview

Package gpuboy for release: switch WASM builds to `--release` mode, minify the browser JS/CSS via esbuild, and add ZIP ROM support so users can open `.zip` archives containing `.gb` files directly from the file picker.

## Requirements

1. WHEN `wasm-pack build` runs (in both `preflight.sh` and CI), THEN it uses the `--release` flag so wasm-opt optimisation runs and output size is reduced.
2. WHEN the user clicks Open ROM and selects a `.zip` file, THEN the app extracts the first `.gb` or `.gbc` entry from the ZIP and loads it as a ROM.
3. WHEN a selected `.zip` contains no `.gb` or `.gbc` file, THEN the error element displays "No .gb or .gbc file found in ZIP." and no ROM is loaded.
4. WHEN CI builds the site, THEN `index.js` and `style.css` are minified with esbuild before upload to GitHub Pages.
5. WHEN `./preflight.sh` runs, THEN it also executes an esbuild minification step as a final validation (requires `esbuild` on PATH).

## Acceptance Criteria

- [ ] `pkg/gpuboy_wasm_bg.wasm` produced by CI is smaller than before (confirms `--release` is active).
- [ ] Opening a `.zip` containing a `.gb` via the file picker loads and runs the game.
- [ ] Opening a `.zip` with no `.gb`/`.gbc` shows "No .gb or .gbc file found in ZIP." in the error element.
- [ ] `_site/www/index.js` in the CI artifact is minified (no leading whitespace, comments stripped).
- [ ] `./preflight.sh` passes (exit 0) with `esbuild` installed.
- [ ] Existing `.gb`/`.gbc` file loading still works (no regression).
- [ ] Sample ROMs in the hamburger menu still load correctly (no regression).

## Design

### Architecture

Three independent improvements bundled in one phase:

**1. wasm-pack --release** — one-line change in `preflight.sh` and `ci.yml`. Enables `wasm-opt` in the wasm-pack pipeline; typical WASM size reduction is 30–50%.

**2. ZIP ROM loading** — uses [fflate](https://github.com/101arrowz/fflate) (vendored ESM build, ~23 KB) for ZIP decompression. fflate exposes `unzipSync(Uint8Array) → { [path: string]: Uint8Array }` and handles DEFLATE and STORED compression methods. The vendor file lives at `www/vendor/fflate.esm.js` and is imported as a sibling ES module — no bundler required for local dev.

**3. esbuild minification** — esbuild minifies `index.js` and `style.css` into `_site/www/` during the CI "Prepare site" step. No bundling: `--minify` without `--bundle` preserves import statements so `../pkg/gpuboy_wasm.js` resolves correctly at runtime in the deployed layout.

### Key Decisions

**No JS bundling.** Bundling `gpuboy_wasm.js` would pull in the WASM binary, requiring an esbuild plugin. `--minify` alone avoids this and keeps the build simple.

**fflate over a hand-written ZIP parser.** fflate is 23 KB, well-tested, and handles the two methods (DEFLATE, STORED) used by every common ZIP tool. A hand-written parser would be ~50 lines but harder to trust.

**First `.gb`/`.gbc` match.** If a ZIP contains multiple ROM files, the first entry found (ZIP central-directory order) is loaded. A single game per ZIP is the common case; no disambiguation UI needed.

**Vendor directory copied in both CI and preflight.** The `www/vendor/` directory must be present in the deployed site. CI copies it to `_site/www/vendor/`. preflight doesn't produce a deployable site, but its esbuild step runs from `www/` source so no copy is needed there.

## Tasks

- [ ] 1. Fetch the fflate ESM browser build and save it as `www/vendor/fflate.esm.js`:
  ```
  mkdir -p www/vendor
  curl -fsSL https://cdn.jsdelivr.net/npm/fflate@0.8.2/esm/browser.js -o www/vendor/fflate.esm.js
  ```
  *(req 2, 3)*

- [ ] 2. Update `www/index.html`: change the `<input id="rom-picker">` `accept` attribute from `.gb,.gbc` to `.gb,.gbc,.zip`. *(req 2)*

- [ ] 3. Update `www/index.js`:
  - Add `import { unzipSync } from './vendor/fflate.esm.js';` at the top of the file (after the existing wasm import).
  - In the `rom-picker` `change` handler, after reading the `ArrayBuffer` into a `Uint8Array`, insert ZIP extraction before calling `loadRomBytes`:
    ```js
    if (file.name.toLowerCase().endsWith('.zip')) {
        const entries = unzipSync(data);
        const key = Object.keys(entries).find(k => /\.(gb|gbc)$/i.test(k));
        if (!key) {
            const errEl = document.getElementById('error');
            if (errEl) {
                errEl.textContent = 'No .gb or .gbc file found in ZIP.';
                errEl.style.display = 'block';
            }
            return;
        }
        data = entries[key];
    }
    ```
  *(req 2, 3)*

- [ ] 4. Update `preflight.sh`:
  - Add `--release` to the wasm-pack command. *(req 1)*
  - Append a final esbuild step that minifies to `dist/www/` (creating the directory if needed):
    ```bash
    echo "==> esbuild (minify)"
    mkdir -p dist/www
    esbuild www/index.js --minify --outfile=dist/www/index.js
    esbuild www/style.css --minify --outfile=dist/www/style.css
    ```
  *(req 5)*

- [ ] 5. Add `dist/` to `.gitignore` if not already present. *(req 5)*

- [ ] 6. Update `.github/workflows/ci.yml`:
  - Add `--release` to the `wasm-pack build` step. *(req 1)*
  - Add an `Install esbuild` step immediately before `Prepare site`:
    ```yaml
    - name: Install esbuild
      run: npm install -g esbuild
    ```
  - Update `Prepare site` to:
    - Copy `www/index.html` and `www/vendor/` to `_site/www/`.
    - Replace the `cp www/index.js www/style.css` lines with esbuild minification:
      ```yaml
      - name: Prepare site
        run: |
          mkdir -p _site/www _site/pkg
          cp www/index.html _site/www/
          cp -r www/vendor _site/www/vendor
          esbuild www/index.js --minify --outfile=_site/www/index.js
          esbuild www/style.css --minify --outfile=_site/www/style.css
          cp -r pkg/. _site/pkg/
          printf '<!DOCTYPE html><html><head><meta http-equiv="refresh" content="0; url=www/"></head></html>\n' > _site/index.html
      ```
  *(req 4)*

## Manual Testing

1. Install esbuild if needed: `npm install -g esbuild`.
2. Run `./preflight.sh`. Confirm exit 0 and PASS.
3. Build and serve: `wasm-pack build crates/gpuboy-wasm --target web --release --out-dir ../../pkg && python -m http.server 8000`.
4. Open `http://localhost:8000/www/` in Chrome.
5. Click the folder icon; select a `.gb` file. Confirm ROM loads and runs.
6. Click the folder icon; select a `.zip` containing a `.gb`. Confirm ROM loads and runs.
7. Click the folder icon; select a `.zip` with no `.gb`/`.gbc` inside. Confirm "No .gb or .gbc file found in ZIP." appears.
8. Verify sample ROMs in the hamburger menu still load.
9. Push to main; inspect the Actions run. Confirm `wasm-pack build` and `Prepare site` succeed. Download the Pages artifact and verify `www/index.js` is minified.

**Green light:** [ ]
