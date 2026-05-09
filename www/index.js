import init, { run, load_rom, init_renderer, render_frame_wgpu, start_audio,
               set_volume, set_paused }
    from "../pkg/gpuboy_wasm.js";

function render2d(ctx2d, fb) {
    const imageData = new ImageData(new Uint8ClampedArray(fb), 160, 144);
    ctx2d.putImageData(imageData, 0, 0);
}

const SAMPLE_ROMS = [
    { name: 'cpu_instrs (Blargg)',  url: 'https://raw.githubusercontent.com/retrio/gb-test-roms/master/cpu_instrs/cpu_instrs.gb' },
    { name: 'mem_timing (Blargg)',  url: 'https://raw.githubusercontent.com/retrio/gb-test-roms/master/mem_timing/mem_timing.gb' },
    { name: 'Fairy Lake (Hacktix)', url: 'https://raw.githubusercontent.com/Hacktix/scribbltests/master/fairylake/fairylake.gb' },
];

let romLoading = false;

async function main() {
    await init();
    run();

    let useWebGpu = false;
    let webGpuAvailable = false;

    try {
        await init_renderer('screen');
        useWebGpu = true;
        webGpuAvailable = true;
    } catch (err) {
        console.error('wgpu init failed:', err);
    }

    const statusEl = document.getElementById('webgpu-status');
    if (statusEl) {
        statusEl.textContent = webGpuAvailable ? 'WebGPU Enabled' : 'WebGPU Disabled';
        statusEl.className   = webGpuAvailable ? 'enabled'        : 'disabled';
    }

    document.getElementById('screen').style.display    = useWebGpu ? 'block' : 'none';
    document.getElementById('screen-2d').style.display = useWebGpu ? 'none'  : 'block';
    const ctx2d = document.getElementById('screen-2d').getContext('2d');

    function loadRomBytes(data) {
        const errEl = document.getElementById('error');
        if (errEl) errEl.style.display = 'none';
        load_rom(data);
        // start_audio is idempotent; creates AudioContext + GainNode + ScriptProcessorNode in Rust.
        // The callback receives a Uint8Array framebuffer once per audio buffer (~10.7×/sec).
        start_audio((fb) => {
            if (useWebGpu) {
                render_frame_wgpu(fb);
            } else {
                render2d(ctx2d, fb);
            }
        });
    }

    // Renderer toggle in hamburger menu
    const menuRenderer = document.getElementById('menu-renderer');
    menuRenderer.textContent = useWebGpu ? 'Switch to 2D canvas' : 'Switch to WebGPU';
    if (webGpuAvailable) {
        menuRenderer.addEventListener('click', () => {
            useWebGpu = !useWebGpu;
            document.getElementById('screen').style.display    = useWebGpu ? 'block' : 'none';
            document.getElementById('screen-2d').style.display = useWebGpu ? 'none'  : 'block';
            menuRenderer.textContent = useWebGpu ? 'Switch to 2D canvas' : 'Switch to WebGPU';
            gbMenu.classList.add('hidden');
        });
    } else {
        menuRenderer.disabled = true;
    }

    // ROM file picker
    document.getElementById('btn-folder').addEventListener('click', () => {
        document.getElementById('rom-picker').click();
    });

    document.getElementById('rom-picker').addEventListener('change', (e) => {
        const file = e.target.files[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onerror = (ev) => console.error('FileReader error:', ev.target.error);
        reader.onload = (ev) => {
            loadRomBytes(new Uint8Array(ev.target.result));
        };
        reader.readAsArrayBuffer(file);
    });

    // Audio toggle
    let audioMuted = false;
    document.getElementById('btn-audio').addEventListener('click', () => {
        audioMuted = !audioMuted;
        set_volume(audioMuted ? 0.0 : 1.0);
        document.getElementById('icon-audio-on').style.display  = audioMuted ? 'none' : '';
        document.getElementById('icon-audio-off').style.display = audioMuted ? ''     : 'none';
    });

    // Play/pause toggle
    let emulatorPaused = false;
    document.getElementById('btn-playpause').addEventListener('click', () => {
        emulatorPaused = !emulatorPaused;
        set_paused(emulatorPaused);
        document.getElementById('icon-pause').style.display = emulatorPaused ? 'none' : '';
        document.getElementById('icon-play').style.display  = emulatorPaused ? ''     : 'none';
    });

    // Hamburger menu
    const gbMenu = document.getElementById('gb-menu');
    document.getElementById('btn-menu').addEventListener('click', (e) => {
        e.stopPropagation();
        gbMenu.classList.toggle('hidden');
    });

    document.addEventListener('click', (e) => {
        if (!gbMenu.contains(e.target) && e.target !== document.getElementById('btn-menu')) {
            gbMenu.classList.add('hidden');
        }
    });

    // Zoom buttons
    document.querySelectorAll('.zoom-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            const zoom = parseInt(btn.dataset.zoom, 10);
            const w = `${160 * zoom}px`;
            const h = `${144 * zoom}px`;
            document.getElementById('screen').style.width     = w;
            document.getElementById('screen').style.height    = h;
            document.getElementById('screen-2d').style.width  = w;
            document.getElementById('screen-2d').style.height = h;
            document.querySelectorAll('.zoom-btn').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            gbMenu.classList.add('hidden');
        });
    });

    // Sample ROM buttons
    const menuRoms = document.getElementById('menu-roms');
    for (const entry of SAMPLE_ROMS) {
        const btn = document.createElement('button');
        btn.textContent = entry.name;
        btn.addEventListener('click', async () => {
            if (romLoading) return;
            romLoading = true;
            btn.textContent = 'Loading…';
            try {
                const resp = await fetch(entry.url);
                if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
                const buf = await resp.arrayBuffer();
                loadRomBytes(new Uint8Array(buf));
                gbMenu.classList.add('hidden');
            } catch (err) {
                const errEl = document.getElementById('error');
                if (errEl) {
                    errEl.textContent = `Failed to load ${entry.name}: ${err.message}`;
                    errEl.style.display = 'block';
                }
            } finally {
                btn.textContent = entry.name;
                romLoading = false;
            }
        });
        menuRoms.appendChild(btn);
    }
}

main().catch((err) => {
    const el = document.getElementById('error');
    if (el) {
        el.textContent = `Failed to load: ${err}`;
        el.style.display = 'block';
    }
});
