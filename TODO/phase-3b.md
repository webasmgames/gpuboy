# Phase 3b: WebGPU Renderer

## Overview

Replaces the Phase 3a `putImageData` 2D canvas rendering path with a WebGPU renderer. The WASM side is unchanged — `gpuboy-wasm` continues to export `get_framebuffer() -> Vec<u8>` as a flat 160×144 RGBA byte array. All WebGPU code lives in `www/index.js`. Each frame the framebuffer is uploaded as a `GPUTexture` and drawn to the canvas via a fullscreen triangle with a passthrough WGSL shader. This phase establishes the WebGPU foundation that future phases (scanline shaders, LCD grid, color correction) will build upon.

## Requirements

1. WHEN the page loads and WebGPU is available (`navigator.gpu` is defined), THEN the emulator initializes a WebGPU adapter, device, and canvas context before entering the render loop.

2. WHEN WebGPU is unavailable (`navigator.gpu` is undefined or `requestAdapter()` returns null), THEN a human-readable error message is displayed in the DOM's `#error` element, the 2D canvas fallback path activates, and no uncaught exception is thrown.

3. WHEN a frame is rendered via WebGPU, THEN the 160×144 RGBA framebuffer returned by `get_framebuffer()` is uploaded to a GPU texture and drawn to the canvas as a fullscreen quad with nearest-neighbor sampling.

4. WHEN the WebGPU renderer is active, THEN the visual output is pixel-perfect: each Game Boy pixel maps to exactly one logical canvas pixel (the canvas is still 160×144, CSS-scaled 3× to 480×432 with `image-rendering: pixelated`), with no blurring or color shifts.

5. WHEN the WebGPU renderer is active, THEN the `putImageData` 2D canvas rendering path is not used.

6. WHEN a WebGPU device error occurs after initialization (e.g. device lost), THEN the error is logged to the browser console via `console.error`.

## Acceptance Criteria

- [ ] Loading a ROM and running the emulator displays game pixels correctly with no visual difference from Phase 3a's output.
- [ ] Pixels are sharp (no bilinear blurring) at 3× CSS scale.
- [ ] Browser DevTools shows WebGPU activity (Chromium: Application > GPU; or the WebGPU API calls appear in the timeline).
- [ ] Deleting `navigator.gpu` before page init (via a DevTools snippet or browser flag) causes the `#error` div to display a WebGPU unavailability message and the emulator falls back to the 2D canvas path and still renders.
- [ ] No uncaught JS exceptions in the console under either the WebGPU or fallback path.
- [ ] The 2D canvas `putImageData` call does not appear anywhere in the WebGPU rendering code path.

## Design

### Architecture

All changes are confined to `www/index.js`. No Rust or WASM changes are needed.

```
www/
  index.js   — WebGPU init, texture/pipeline setup, per-frame renderFrame(); 2D fallback
  index.html — unchanged
crates/
  gpuboy-wasm/   — unchanged; still exports get_framebuffer() -> Vec<u8>
  gpuboy-core/   — unchanged
```

The JS module is structured as three layers:

1. **WASM layer** — existing: `init()`, `run()`, `load_rom()`, `step_frame()`, `get_framebuffer()`
2. **Renderer layer** — new: `initWebGPU()` returns a renderer object `{ renderFrame(fb) }`, or null on failure
3. **Main loop** — updated: after each `step_frame()` call, dispatch to the renderer object's `renderFrame` if WebGPU succeeded, otherwise use the 2D canvas fallback

WebGPU setup sequence (all in `initWebGPU()`):
```
navigator.gpu.requestAdapter()
  → adapter.requestDevice()
  → canvas.getContext('webgpu')
  → context.configure({ device, format })
  → create GPUTexture (160×144, rgba8unorm, reused each frame)
  → create GPUSampler (magFilter: 'nearest', minFilter: 'nearest')
  → create GPURenderPipeline (vertex + fragment WGSL)
  → create GPUBindGroupLayout, GPUBindGroup
  → return { renderFrame }
```

Per-frame `renderFrame(framebuffer)`:
```
device.queue.writeTexture(...)   // upload framebuffer bytes
encoder = device.createCommandEncoder()
pass = encoder.beginRenderPass({ colorAttachments: [{ view: context.getCurrentTexture().createView(), ... }] })
pass.setPipeline(pipeline)
pass.setBindGroup(0, bindGroup)
pass.draw(3)                     // fullscreen triangle, 3 vertices, no vertex buffer
pass.end()
device.queue.submit([encoder.finish()])
```

### Data Structures

No new Rust types. The JS renderer state is captured in the closure returned by `initWebGPU()`:

```js
// State owned by the closure:
let device         // GPUDevice
let context        // GPUCanvasContext
let pipeline       // GPURenderPipeline
let bindGroup      // GPUBindGroup
let texture        // GPUTexture — reused each frame
let sampler        // GPUSampler — created once
```

The framebuffer is a `Uint8Array` of length `160 * 144 * 4 = 92160` bytes in RGBA order, exactly as returned by `get_framebuffer()`.

### WGSL Shaders

Both shaders are defined as inline JS template literal strings and compiled into a single `GPUShaderModule`.

**Vertex shader** — fullscreen triangle trick, no vertex buffer needed:

```wgsl
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0,  3.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
    );
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, -1.0),
        vec2<f32>(0.0,  1.0),
        vec2<f32>(2.0,  1.0),
    );
    var out: VsOut;
    out.position = vec4<f32>(pos[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}
```

UV convention: `(0,0)` = top-left, `(1,1)` = bottom-right of the canvas. The fullscreen triangle extends beyond the clip boundary (the GPU clips it), so UVs must cover exactly `[0,1]×[0,1]` at the canvas edges. Passing UVs as a vertex output avoids the fragile `frag_pos / canvas_size` division in the fragment shader.

Note: The UV values at the vertex positions themselves are outside [0,1] (v=-1 at vertex 0, u=2 at vertex 2). This is intentional. The GPU clips the triangle to the NDC square, and linear interpolation produces UVs in [0,1] exactly at the canvas corners. Do not "fix" the out-of-range values — they are required for the math to work.

**Fragment shader:**

```wgsl
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
```

**Struct and binding declarations (combined shader module):**

```wgsl
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
```

### Key Decisions

**All WebGPU in JS, not Rust.** The `wgpu` Rust crate is large, pulls in heavy dependencies, and targets native + WASM with a high configuration burden. Doing it in JS keeps WASM binary size down, avoids re-architecture of `gpuboy-wasm`, and lets Phase 3b ship faster. Future shader phases can stay in JS as well.

**Texture reused each frame, not recreated.** Creating a `GPUTexture` each frame allocates GPU memory on every call. Instead, allocate once in `initWebGPU()` and call `writeTexture` each frame. This is the standard pattern for streaming video/emulator output.

**Fullscreen triangle, not quad.** A single triangle with 3 vertices requires no index buffer and no vertex buffer. It produces a perfectly rasterized fullscreen fill. The standard trick is to use clip-space coordinates `(-1,3), (-1,-1), (3,-1)` which form a triangle that exactly covers the NDC square `[-1,1]×[-1,1]`.

**UVs passed via vertex output.** Computing UVs from `frag_pos / vec2(canvas_width, canvas_height)` in the fragment shader requires knowing the canvas size as a uniform or hardcoded constant. Passing UVs from the vertex shader is cleaner and avoids an extra uniform buffer.

**Nearest-neighbor sampler.** `magFilter: 'nearest'` and `minFilter: 'nearest'` preserve pixel-art aesthetics at the 3× CSS scale. Bilinear would blur pixels and contradict `image-rendering: pixelated`.

**Canvas format via `getPreferredCanvasFormat()`.** Using the device's preferred format (`bgra8unorm` on most desktop, `rgba8unorm` on some mobile) avoids a GPU-side format conversion on every present. The texture format used for the framebuffer remains `rgba8unorm` regardless; the GPU handles the blit to the preferred format.

**Graceful fallback.** If `navigator.gpu` is undefined (Firefox without a flag, Safari < 18, or old Chromium) or `requestAdapter()` returns null (no suitable GPU), the code catches the failure, displays a message in `#error`, and activates the 2D canvas `putImageData` path. The emulator continues to run — only the renderer differs. This allows testing on browsers without WebGPU.

**`device.lost` promise.** WebGPU devices can be lost asynchronously (GPU reset, tab backgrounded on mobile). The code attaches a `.then` handler to `device.lost` that logs via `console.error`. Recovery (re-init) is out of scope for Phase 3b. After device loss, subsequent WebGPU API calls on the lost device will generate GPU validation errors visible in DevTools but will not throw JS exceptions. The rAF loop will continue without crashing. This is accepted behavior for Phase 3b — recovery/re-init is out of scope.

**Bind group created once.** Since the texture is reused each frame, the bind group referencing its view can also be created once. This avoids creating a new `GPUBindGroup` on every frame.

## Tasks

- [ ] 1. In `www/index.js`, update the `import` statement at the top of the file to include `step_frame` and `get_framebuffer` from `../pkg/gpuboy_wasm.js`:
    ```js
    import init, { run, load_rom, step_frame, get_framebuffer } from "../pkg/gpuboy_wasm.js";
    ```
    Note: `step_frame` and `get_framebuffer` are Phase 3a WASM exports that must already exist. Then, extract the existing Phase 3a per-frame rendering into a named function `function render2d(ctx2d, framebuffer)` that calls `ctx2d.putImageData(...)`. This function becomes the 2D fallback path and makes the later switchover clean. *(req 2, 5)*

- [ ] 2. In `www/index.js`, add a feature-detection guard at the top of `initWebGPU()`: if `!navigator.gpu`, throw an `Error('WebGPU not available: navigator.gpu is undefined')`. Callers catch this and fall back. *(req 2)*

- [ ] 3. Implement `async function initWebGPU(canvas)` in `www/index.js`. Perform the following steps in order, throwing descriptive `Error`s on failure:
    - `const adapter = await navigator.gpu.requestAdapter()`; if `adapter` is null, throw `Error('WebGPU not available: no suitable adapter')`.
    - `const device = await adapter.requestDevice()`.
    - Attach `device.lost.then(info => console.error('WebGPU device lost:', info.message, info.reason))` for req 6.
    - `const context = canvas.getContext('webgpu')`; if null, throw `Error('Failed to get WebGPU canvas context')`. Note: calling `getContext('webgpu')` only after the device is confirmed avoids permanently locking the canvas if earlier init steps fail.
    - `const format = navigator.gpu.getPreferredCanvasFormat()`.
    - `context.configure({ device, format })`.
    - Create `texture`: `device.createTexture({ size: [160, 144], format: 'rgba8unorm', usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST })`.
    - Create `sampler`: `device.createSampler({ magFilter: 'nearest', minFilter: 'nearest' })`.
    - Return `{ device, context, format, texture, sampler }`. *(req 1, 3, 6)*

- [ ] 4. In `www/index.js`, define the combined WGSL shader source as a JS template literal constant `SHADER_SRC`. The single module must contain the `VsOut` struct, both `@group(0)` binding declarations, `vs_main`, and `fs_main` exactly as written in the Design §WGSL Shaders section. The vertex entry point is `vs_main`, the fragment entry point is `fs_main`. *(req 3)*

- [ ] 5. Implement `createPipeline(device, format, texture, sampler)` in `www/index.js`. Steps:
    - `const shaderModule = device.createShaderModule({ code: SHADER_SRC })`.
    - `const bindGroupLayout = device.createBindGroupLayout({ entries: [ { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'float' } }, { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: { type: 'filtering' } } ] })`.
    - `const pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [bindGroupLayout] })`.
    - `const pipeline = device.createRenderPipeline({ layout: pipelineLayout, vertex: { module: shaderModule, entryPoint: 'vs_main' }, fragment: { module: shaderModule, entryPoint: 'fs_main', targets: [{ format }] }, primitive: { topology: 'triangle-list' } })`.
    - `const bindGroup = device.createBindGroup({ layout: bindGroupLayout, entries: [ { binding: 0, resource: texture.createView() }, { binding: 1, resource: sampler } ] })`.
    - Return `{ pipeline, bindGroup }`. *(req 3, 4)*

- [ ] 6. Implement `function renderFrameWebGPU({ device, context, texture, pipeline, bindGroup }, framebuffer)` in `www/index.js`. Steps:
    - Upload framebuffer: `device.queue.writeTexture( { texture }, framebuffer, { bytesPerRow: 160 * 4 }, [160, 144] )`.
    - `const encoder = device.createCommandEncoder()`.
    - `const pass = encoder.beginRenderPass({ colorAttachments: [{ view: context.getCurrentTexture().createView(), loadOp: 'dont-care', storeOp: 'store' }] })`. (The swapchain texture changes every frame, so `getCurrentTexture().createView()` must be called per-frame. This is different from the framebuffer texture view used in the bind group, which is stable because the texture object is reused.) The fullscreen triangle covers every pixel, so a clear is wasted work. `dont-care` tells the GPU the initial contents are irrelevant.
    - `pass.setPipeline(pipeline)`.
    - `pass.setBindGroup(0, bindGroup)`.
    - `pass.draw(3)`.
    - `pass.end()`.
    - `device.queue.submit([encoder.finish()])`. *(req 3, 4, 5)*

- [ ] 7. Update `async function main()` in `www/index.js` to wire everything together:
    - After `await init()` and `run()`, attempt WebGPU init:
      ```js
      let renderer = null;
      try {
          const canvas = document.getElementById('screen');
          const gpuState = await initWebGPU(canvas);
          const pipelineState = createPipeline(gpuState.device, gpuState.format, gpuState.texture, gpuState.sampler);
          renderer = { ...gpuState, ...pipelineState };
      } catch (err) {
          console.error('WebGPU init failed:', err);
          const errEl = document.getElementById('error');
          if (errEl) {
              errEl.textContent = `WebGPU unavailable: ${err.message}. Falling back to 2D canvas.`;
              errEl.style.display = 'block';
          }
      }
      ```
    - For the 2D fallback, acquire the 2D context once if `renderer` is null:
      ```js
      const ctx2d = renderer ? null : document.getElementById('screen').getContext('2d');
      if (!renderer && !ctx2d) {
          const errEl = document.getElementById('error');
          if (errEl) errEl.textContent = 'Canvas context unavailable: could not get 2D context (canvas may be locked to WebGPU).';
          return; // abort main()
      }
      ```
    - Set up the `requestAnimationFrame` loop and ROM loading. The expected structure is:
      ```js
      let animationId = null;

      function loop() {
          step_frame();
          const fb = get_framebuffer();
          if (renderer) {
              renderFrameWebGPU(renderer, fb);
          } else {
              render2d(ctx2d, fb);
          }
          animationId = requestAnimationFrame(loop);
      }

      // Start loop after ROM is loaded
      document.getElementById('rom-input').addEventListener('change', (e) => {
          const file = e.target.files[0];
          if (!file) return;
          const reader = new FileReader();
          reader.onload = (ev) => {
              const data = new Uint8Array(ev.target.result);
              load_rom(data);
              if (animationId !== null) cancelAnimationFrame(animationId);
              loop();
          };
          reader.readAsArrayBuffer(file);
      });
      ```
      Note: the exact ROM loading UI may differ in Phase 3a's implementation; adapt this to match whatever `index.js` structure Phase 3a produced.
    *(req 1, 2, 5)*

- [ ] 8. Verify that no `putImageData` call exists in the WebGPU code path. The `render2d` function (fallback only) is the sole remaining user of `putImageData`. In the main loop, `putImageData` is never called when `renderer` is non-null. *(req 5)*

- [ ] 9. Remove any now-dead Phase 3a-specific code that was replaced: if Phase 3a had a direct `ctx.putImageData` call inline in the main loop (rather than in `render2d`), replace it with the dispatch from task 7. Ensure `render2d` is defined before `main()` is called. *(req 5)*

## Manual Testing

1. Build the WASM: `wasm-pack build crates/gpuboy-wasm --target web`.
2. Serve locally: `python -m http.server 8000`, open `http://localhost:8000/www/` in Chrome or Edge (WebGPU-enabled browser).
3. Open DevTools console. Confirm no errors on page load. Confirm "gpuboy ready" appears.
4. Load a flat (MBC-0) .gb ROM using the file picker. Confirm the ROM title appears in the console and game pixels render on the canvas.
5. Verify pixels are sharp, not blurry — individual pixels should have hard edges at the 3× CSS scale. Zoom into the canvas in the browser to confirm.
6. Open DevTools > Performance (or use `chrome://gpu`) and confirm WebGPU-related activity (draw calls, GPU command submission). In Chrome 113+, `chrome://gpu` → "Graphics Feature Status" → "WebGPU" should show "Hardware accelerated".
7. Test the fallback path using one of these methods:
   - **Browser flags**: In Chrome/Edge, navigate to `chrome://flags/#enable-unsafe-webgpu` and disable WebGPU, then reload the page. Confirm the `#error` div is visible and the 2D canvas fallback renders.
   - **Alternative browser**: Open the page in Firefox (without WebGPU enabled via `dom.webgpu.enabled` flag). Confirm fallback behavior.
   - **Programmatic**: Temporarily add `if (true) throw new Error('WebGPU disabled for test')` as the first line of `initWebGPU()`, reload, confirm fallback, then remove the line.
8. Open the page in Firefox (without WebGPU flags enabled) and confirm the same fallback behavior (if not already covered by step 7).
9. Load a ROM that produces known visual output (e.g. a title screen or test pattern). Compare the WebGPU-rendered output side-by-side against a screenshot from Phase 3a's `putImageData` path to confirm pixel-identical output.

**Green light:** [ ]
