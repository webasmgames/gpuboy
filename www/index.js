import init, { run, load_rom, get_framebuffer, init_renderer, render_frame_wgpu, start_audio }
    from "../pkg/gpuboy_wasm.js";

function render2d(ctx2d, fb) {
    const imageData = new ImageData(new Uint8ClampedArray(fb), 160, 144);
    ctx2d.putImageData(imageData, 0, 0);
}

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

    document.getElementById('screen').style.display    = useWebGpu ? 'block' : 'none';
    document.getElementById('screen-2d').style.display = useWebGpu ? 'none'  : 'block';
    const ctx2d = document.getElementById('screen-2d').getContext('2d');

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

    document.getElementById('rom-picker').addEventListener('change', (e) => {
        const file = e.target.files[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onerror = (ev) => console.error('FileReader error:', ev.target.error);
        reader.onload = (ev) => {
            const data = new Uint8Array(ev.target.result);
            load_rom(data);
            // start_audio is idempotent; creates AudioContext + ScriptProcessorNode in Rust.
            // The callback receives a Uint8Array framebuffer once per audio buffer (~10.7×/sec).
            start_audio((fb) => {
                if (useWebGpu) {
                    render_frame_wgpu(fb);
                } else {
                    render2d(ctx2d, fb);
                }
            });
        };
        reader.readAsArrayBuffer(file);
    });
}

main().catch((err) => {
    const el = document.getElementById('error');
    if (el) {
        el.textContent = `Failed to load: ${err}`;
        el.style.display = 'block';
    }
});
