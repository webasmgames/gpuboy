# Phase 3b: WebGPU Renderer (Rust/wgpu)

## Overview

Replaces the Phase 3a `putImageData` 2D canvas rendering path with a Rust/wgpu WebGPU renderer. Emulator logic in `gpuboy-core` is unchanged. A new `gpuboy-render` crate wraps `wgpu` and exposes a `WgpuRenderer` struct. `gpuboy-wasm` gets two new exports: `init_renderer(canvas_id: &str) -> Promise` (async) and `render_frame_wgpu(fb: &[u8])`. The JS shell tries wgpu first; on failure it falls back to the Phase 3a `putImageData` path. A toggle button allows switching between renderers at runtime (two canvases, one per renderer, swapped via CSS `display`).

## Requirements

1. WHEN the page loads and WebGPU is available, THEN the emulator initializes a wgpu adapter, device, and canvas surface before entering the render loop.

2. WHEN WebGPU is unavailable (adapter request returns None) or `init_renderer` rejects, THEN a human-readable error message is displayed in `#error`, the 2D canvas fallback activates, and no uncaught exception is thrown.

3. WHEN a frame is rendered via WebGPU, THEN the 160×144 RGBA framebuffer is uploaded to a GPU texture and drawn to the canvas as a fullscreen triangle with nearest-neighbor sampling.

4. WHEN the WebGPU renderer is active, THEN the visual output is pixel-perfect: each Game Boy pixel maps to exactly one logical canvas pixel (160×144 canvas, CSS-scaled 3× to 480×432 with `image-rendering: pixelated`), with no blurring or color shifts.

5. WHEN the WebGPU renderer is active, THEN the `putImageData` 2D canvas rendering path is not used.

6. WHEN a WebGPU device error occurs after initialization, THEN the error is logged via `console.error`.

7. WHEN both renderers have initialized successfully, THEN a toggle button is visible and clicking it switches the active renderer without reloading the page.

## Acceptance Criteria

- [ ] Loading a ROM and running the emulator displays game pixels correctly via the wgpu path.
- [ ] Pixels are sharp (no bilinear blurring) at 3× CSS scale.
- [ ] Browser DevTools shows WebGPU activity.
- [ ] When wgpu init fails (simulated by returning early from `WgpuRenderer::new`), `#error` shows a message and the 2D canvas fallback renders correctly with no uncaught exceptions.
- [ ] `putImageData` is never called in the wgpu render path.
- [ ] Toggle button (visible only when wgpu initialized) switches between wgpu and 2D canvas at runtime; both paths render correctly.

## Design

### Architecture

```
Cargo workspace:
  crates/
    gpuboy-core/     — unchanged
    gpuboy-render/   — NEW: WgpuRenderer (wgpu crate, WASM-only lib)
    gpuboy-wasm/     — thin WASM boundary; adds init_renderer + render_frame_wgpu exports
  www/
    index.html       — adds #screen-2d canvas + toggle button
    index.js         — tries wgpu, falls back to 2D, toggle logic
```

`gpuboy-render` is a plain Rust `lib` crate (not a cdylib). Its entire body is gated with `#![cfg(target_arch = "wasm32")]` so it compiles only when building for WASM. `gpuboy-wasm` stores the renderer in a second `thread_local! { static }` alongside the existing `EMULATOR` static.

### `gpuboy-render` Cargo.toml

```toml
[package]
name = "gpuboy-render"
version = "0.1.0"
edition = "2021"

[lib]
name = "gpuboy_render"

[target.'cfg(target_arch = "wasm32")'.dependencies]
wgpu = { version = "0.20", default-features = false, features = ["wgsl", "webgpu"] }
web-sys = { version = "0.3", features = ["HtmlCanvasElement"] }

[lints]
workspace = true
```

Use the latest compatible `wgpu` version; `"0.20"` is the minimum known to expose `SurfaceTarget::Canvas` and `on_uncaptured_error`. Verify feature flag names against the installed version's docs — the two required features are WGSL shader support and the WebGPU backend.

### `WgpuRenderer` struct

```rust
pub struct WgpuRenderer {
    device:     wgpu::Device,
    queue:      wgpu::Queue,
    surface:    wgpu::Surface<'static>,
    pipeline:   wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    texture:    wgpu::Texture,
}
```

### `WgpuRenderer::new` setup sequence

`pub async fn new(canvas: web_sys::HtmlCanvasElement) -> Result<Self, String>`

Execute these steps in order; return `Err(description)` on any failure:

1. `let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends: wgpu::Backends::BROWSER_WEBGPU, ..Default::default() });`
2. `let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas)).map_err(|e| e.to_string())?;`
3. `let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), ..Default::default() }).await.ok_or_else(|| "no suitable WebGPU adapter".to_string())?;`
4. `let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default(), None).await.map_err(|e| e.to_string())?;`
5. Attach error handler (req 6):
   ```rust
   device.on_uncaptured_error(Box::new(|e| {
       web_sys::console::error_1(&format!("WebGPU device error: {:?}", e).into());
   }));
   ```
6. `let caps = surface.get_capabilities(&adapter);`
   `let format = caps.formats[0];`
7. Configure surface:
   ```rust
   surface.configure(&device, &wgpu::SurfaceConfiguration {
       usage:    wgpu::TextureUsages::RENDER_ATTACHMENT,
       format,
       width:    160,
       height:   144,
       present_mode: wgpu::PresentMode::Fifo,
       alpha_mode:   caps.alpha_modes[0],
       view_formats: vec![],
       desired_maximum_frame_latency: 2,
   });
   ```
8. Create framebuffer texture:
   ```rust
   let texture = device.create_texture(&wgpu::TextureDescriptor {
       label: None,
       size: wgpu::Extent3d { width: 160, height: 144, depth_or_array_layers: 1 },
       mip_level_count: 1,
       sample_count:    1,
       dimension:       wgpu::TextureDimension::D2,
       format:          wgpu::TextureFormat::Rgba8Unorm,
       usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
       view_formats: &[],
   });
   ```
9. Create nearest sampler:
   ```rust
   let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
       mag_filter: wgpu::FilterMode::Nearest,
       min_filter: wgpu::FilterMode::Nearest,
       ..Default::default()
   });
   ```
10. Create shader module: `device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()) })`.
11. Create bind group layout:
    ```rust
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding:    0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled:   false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding:    1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    ```
12. `let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&bgl], push_constant_ranges: &[] });`
13. Create render pipeline:
    ```rust
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label:  None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module:      &shader,
            entry_point: Some("vs_main"),
            buffers:     &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module:      &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend:      None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample:   wgpu::MultisampleState::default(),
        multiview:     None,
        cache:         None,
    });
    ```
    Note: `entry_point: Some(...)` is the wgpu 0.20+ API. Earlier versions use `entry_point: "..."` (a `&str`). `compilation_options` and `cache` were added in 0.20; omit them if using an older version.
14. Create bind group:
    ```rust
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label:  None,
        layout: &bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&texture.create_view(&Default::default())) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
        ],
    });
    ```
15. Return `Ok(WgpuRenderer { device, queue, surface, pipeline, bind_group, texture })`.

### `WgpuRenderer::render_frame` sequence

`pub fn render_frame(&self, fb: &[u8])` — `fb` is `160 * 144 * 4 = 92160` bytes, RGBA.

1. Upload framebuffer:
   ```rust
   self.queue.write_texture(
       wgpu::ImageCopyTexture {
           texture:  &self.texture,
           mip_level: 0,
           origin:   wgpu::Origin3d::ZERO,
           aspect:   wgpu::TextureAspect::All,
       },
       fb,
       wgpu::ImageDataLayout {
           offset:         0,
           bytes_per_row:  Some(160 * 4),
           rows_per_image: None,
       },
       wgpu::Extent3d { width: 160, height: 144, depth_or_array_layers: 1 },
   );
   ```
2. `let frame = self.surface.get_current_texture().expect("surface texture");`
3. `let view = frame.texture.create_view(&Default::default());`
4. `let mut encoder = self.device.create_command_encoder(&Default::default());`
5. Begin render pass:
   ```rust
   let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
       label: None,
       color_attachments: &[Some(wgpu::RenderPassColorAttachment {
           view:           &view,
           resolve_target: None,
           ops: wgpu::Operations {
               load:  wgpu::LoadOp::Clear(wgpu::Color::BLACK), // DontCare not available in all versions; Clear is safe
               store: wgpu::StoreOp::Store,
           },
       })],
       depth_stencil_attachment: None,
       timestamp_writes:         None,
       occlusion_query_set:      None,
   });
   ```
   Note on `LoadOp`: the fullscreen triangle covers every pixel so the clear color is never visible. Use `wgpu::LoadOp::Clear(wgpu::Color::BLACK)` for compatibility; some wgpu versions lack `LoadOp::DontCare`.
6. `pass.set_pipeline(&self.pipeline);`
7. `pass.set_bind_group(0, &self.bind_group, &[]);`
8. `pass.draw(0..3, 0..1);`
9. `drop(pass);`
10. `self.queue.submit(std::iter::once(encoder.finish()));`
11. `frame.present();`

### WGSL Shaders

Define as `const SHADER_SRC: &str` in `gpuboy-render/src/lib.rs`. Identical to the JS design — fullscreen triangle, UVs as vertex output. UV values at the three vertices are intentionally outside [0,1]; the GPU clips the triangle and interpolated UVs are exactly [0,1] at the canvas corners. Do not "fix" the out-of-range values.

```wgsl
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
```

### `gpuboy-wasm` additions

**New deps to add to `crates/gpuboy-wasm/Cargo.toml`:**
```toml
gpuboy-render        = { path = "../gpuboy-render" }
wasm-bindgen-futures = "0.4"
js-sys               = "0.3"
```
Also extend the existing `web-sys` features list to include `"Window"`, `"Document"`, `"HtmlCanvasElement"`.

**New exports to add to `crates/gpuboy-wasm/src/lib.rs`:**

Add at the top alongside existing imports:
```rust
use gpuboy_render::WgpuRenderer;
use wasm_bindgen::JsCast;
```

Add a second thread-local static below `EMULATOR`:
```rust
thread_local! {
    static RENDERER: RefCell<Option<WgpuRenderer>> = const { RefCell::new(None) };
}
```

Add two new `#[wasm_bindgen]` functions (keep all existing functions unchanged):

```rust
#[wasm_bindgen]
pub fn init_renderer(canvas_id: &str) -> js_sys::Promise {
    let canvas_id = canvas_id.to_string();
    wasm_bindgen_futures::future_to_promise(async move {
        let window = web_sys::window()
            .ok_or_else(|| JsValue::from_str("no window"))?;
        let document = window.document()
            .ok_or_else(|| JsValue::from_str("no document"))?;
        let canvas: web_sys::HtmlCanvasElement = document
            .get_element_by_id(&canvas_id)
            .ok_or_else(|| JsValue::from_str(&format!("#{} not found", canvas_id)))?
            .dyn_into()
            .map_err(|_| JsValue::from_str("element is not a canvas"))?;
        let renderer = WgpuRenderer::new(canvas).await
            .map_err(|e| JsValue::from_str(&e))?;
        RENDERER.with(|r| *r.borrow_mut() = Some(renderer));
        Ok(JsValue::undefined())
    })
}

#[wasm_bindgen]
pub fn render_frame_wgpu(fb: &[u8]) {
    RENDERER.with(|r| {
        if let Some(renderer) = r.borrow().as_ref() {
            renderer.render_frame(fb);
        }
    });
}
```

### HTML changes (`www/index.html`)

Add a second canvas and a toggle button. Existing `#screen` becomes the WebGPU canvas (unchanged). Add:

```html
<canvas id="screen-2d" width="160" height="144" style="display: none;"></canvas>
<button id="renderer-toggle" style="display: none;">Switch to 2D canvas</button>
```

Place `#screen-2d` immediately after `#screen`, and the button after both canvases, before `#error`.

Add `#screen-2d` to the CSS rule so it shares the same size and pixelated rendering:
```css
#screen, #screen-2d { width: 480px; height: 432px; image-rendering: pixelated; }
```

### JS changes (`www/index.js`)

Updated import line:
```js
import init, { run, load_rom, step_frame, get_framebuffer, init_renderer, render_frame_wgpu }
    from "../pkg/gpuboy_wasm.js";
```

Keep `render2d(ctx2d, fb)` (the 2D fallback, sole user of `putImageData`) unchanged.

`main()` structure:

```js
async function main() {
    await init();
    run();

    // Attempt WebGPU init via Rust/wgpu
    let useWebGpu = false;
    try {
        await init_renderer('screen');
        useWebGpu = true;
    } catch (err) {
        console.error('wgpu init failed:', err);
        const errEl = document.getElementById('error');
        if (errEl) {
            errEl.textContent = `WebGPU unavailable: ${err}. Falling back to 2D canvas.`;
            errEl.style.display = 'block';
        }
    }

    // Show the active canvas, hide the other
    document.getElementById('screen').style.display    = useWebGpu ? 'block' : 'none';
    document.getElementById('screen-2d').style.display = useWebGpu ? 'none'  : 'block';

    // 2D context — always acquired (the 2D canvas is never used as a webgpu context)
    const ctx2d = document.getElementById('screen-2d').getContext('2d');

    // Toggle button — only shown when wgpu succeeded
    if (useWebGpu) {
        const btn = document.getElementById('renderer-toggle');
        btn.style.display = 'inline';
        btn.addEventListener('click', () => {
            useWebGpu = !useWebGpu;
            document.getElementById('screen').style.display    = useWebGpu ? 'block' : 'none';
            document.getElementById('screen-2d').style.display = useWebGpu ? 'none'  : 'block';
            btn.textContent = useWebGpu ? 'Switch to 2D canvas' : 'Switch to WebGPU';
        });
    }

    let animationId = null;

    function loop() {
        step_frame();
        const fb = get_framebuffer();
        if (useWebGpu) {
            render_frame_wgpu(fb);
        } else {
            render2d(ctx2d, fb);
        }
        animationId = requestAnimationFrame(loop);
    }

    document.getElementById('rom-picker').addEventListener('change', (e) => {
        const file = e.target.files[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onerror = (ev) => console.error('FileReader error:', ev.target.error);
        reader.onload = (ev) => {
            const data = new Uint8Array(ev.target.result);
            load_rom(data);
            if (animationId !== null) cancelAnimationFrame(animationId);
            loop();
        };
        reader.readAsArrayBuffer(file);
    });
}
```

### Key Decisions

**Rust/wgpu instead of JS WebGPU.** All GPU logic lives in Rust — consistent with the rest of the emulator. The `wgpu` crate provides a safe, typed API over raw WebGPU. Future shader phases (scanline effects, LCD grid) stay in Rust.

**New `gpuboy-render` crate.** Keeps `gpuboy-wasm` thin per CLAUDE.md. wgpu is a heavy dependency; a dedicated crate isolates it cleanly.

**`#![cfg(target_arch = "wasm32")]` on the whole crate.** `gpuboy-render` is WASM-only. The crate-level cfg gate prevents `cargo clippy` on the host from trying to compile wgpu without the right backend features.

**`render_frame_wgpu(fb: &[u8])` takes the framebuffer as a parameter.** Keeps `gpuboy-render` decoupled from `gpuboy-core`. JS calls `get_framebuffer()` then `render_frame_wgpu(fb)` — explicit and debuggable. `get_framebuffer()` is retained for the 2D fallback path.

**Two canvases for runtime toggling.** A canvas element's context type is locked on first `getContext()` call — you cannot switch between `'webgpu'` and `'2d'` on the same element. Two canvases (`#screen` locked to WebGPU by wgpu, `#screen-2d` locked to 2D by JS) allow true runtime switching via CSS `display` toggling. The 2D context is acquired once at startup regardless; it just isn't drawn to until the toggle is used.

**`thread_local! { static RENDERER }` pattern.** Matches the existing `EMULATOR` pattern in `gpuboy-wasm`. WASM is single-threaded; `RefCell` is sufficient, no `Mutex` needed.

## Tasks

- [x] 1. In the root `Cargo.toml`, add `"crates/gpuboy-render"` to the `[workspace] members` list. *(architecture)*

- [x] 2. Create `crates/gpuboy-render/Cargo.toml` with the content from §`gpuboy-render` Cargo.toml above. *(architecture)*

- [x] 3. Create `crates/gpuboy-render/src/lib.rs`. Start with `#![cfg(target_arch = "wasm32")]`. Define `const SHADER_SRC: &str` with the full WGSL from §WGSL Shaders. Define the `WgpuRenderer` struct with the six fields from §`WgpuRenderer` struct. *(req 3, 4)*

- [x] 4. Implement `impl WgpuRenderer { pub async fn new(canvas: web_sys::HtmlCanvasElement) -> Result<Self, String> }` following the 15-step sequence in §`WgpuRenderer::new` setup sequence exactly. *(req 1, 3, 6)*

- [x] 5. Implement `impl WgpuRenderer { pub fn render_frame(&self, fb: &[u8]) }` following the 11-step sequence in §`WgpuRenderer::render_frame` sequence. *(req 3, 4, 5)*

- [x] 6. Update `crates/gpuboy-wasm/Cargo.toml`: add `gpuboy-render`, `wasm-bindgen-futures`, and `js-sys` dependencies; add `"Window"`, `"Document"`, `"HtmlCanvasElement"` to the `web-sys` features list. See §`gpuboy-wasm` additions for the exact additions. *(architecture)*

- [x] 7. Update `crates/gpuboy-wasm/src/lib.rs`: add `use gpuboy_render::WgpuRenderer` and `use wasm_bindgen::JsCast`; add the `RENDERER` thread-local static; add `init_renderer` and `render_frame_wgpu` exports. Keep all existing code unchanged. See §`gpuboy-wasm` additions for the exact code. *(req 1, 2, 3)*

- [x] 8. Update `www/index.html`: add `#screen-2d` canvas and `#renderer-toggle` button; update the CSS selector to cover both canvases. See §HTML changes for the exact additions. *(req 7)*

- [x] 9. Rewrite `www/index.js` following §JS changes: updated import, keep `render2d`, update `main()` to try wgpu, show/hide canvases, wire toggle button, dispatch in loop. *(req 1, 2, 5, 7)*

- [x] 10. Verify `putImageData` does not appear in the wgpu code path: it must only be called inside `render2d()`, which is only called in the loop when `useWebGpu` is false. *(req 5)*

## Manual Testing

1. Build: `wasm-pack build crates/gpuboy-wasm --target web`.
2. Serve: `python -m http.server 8000`, open `http://localhost:8000/www/` in Chrome/Edge.
3. Open DevTools console. Confirm no errors on load.
4. Load a flat (MBC-0) `.gb` ROM. Confirm pixels render on the `#screen` canvas via the wgpu path.
5. Verify pixels are sharp (hard pixel edges) at 3× CSS scale.
6. DevTools → `chrome://gpu` → confirm WebGPU is hardware accelerated.
7. Confirm "Switch to 2D canvas" button is visible. Click it; confirm `#screen-2d` is now visible and renders the same output. Click again; confirm back to wgpu.
8. Test fallback: add `return Err("disabled for test".to_string());` as the first line of `WgpuRenderer::new`, rebuild. Confirm `#error` shows a message, `#screen-2d` is visible, 2D canvas renders, toggle button is hidden. Remove the test line.
9. Pixel-compare both renderers by toggling while a ROM runs — output should look identical.

**Green light:** [x]
