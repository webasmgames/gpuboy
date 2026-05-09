import init, { run, load_rom, init_renderer, render_frame_wgpu, start_audio,
               set_volume, set_paused, set_buttons, extract_zip_rom }
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
            let data = new Uint8Array(ev.target.result);
            if (file.name.toLowerCase().endsWith('.zip')) {
                try {
                    data = extract_zip_rom(data);
                } catch (err) {
                    const errEl = document.getElementById('error');
                    if (errEl) {
                        errEl.textContent = String(err);
                        errEl.style.display = 'block';
                    }
                    return;
                }
            }
            loadRomBytes(data);
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

    // ── Input ──────────────────────────────────────────────────────────────────

    let joyBits = 0;
    let touchBits = 0;
    let gamepadBits = 0;

    // bit0=A bit1=B bit2=Sel bit3=Start bit4=Right bit5=Left bit6=Up bit7=Down
    const KEY_MAP = {
        'KeyW':       1 << 6,
        'KeyA':       1 << 5,
        'KeyS':       1 << 7,
        'KeyD':       1 << 4,
        'ArrowRight': 1 << 0,
        'ArrowLeft':  1 << 1,
        'ArrowDown':  1 << 3,
        'ArrowUp':    1 << 2,
    };

    function syncButtons() {
        set_buttons(joyBits | touchBits | gamepadBits);
    }

    document.addEventListener('keydown', (e) => {
        if (document.activeElement && document.activeElement.tagName === 'INPUT') return;
        const bit = KEY_MAP[e.code];
        if (bit !== undefined) {
            e.preventDefault();
            joyBits |= bit;
            syncButtons();
        }
    });

    document.addEventListener('keyup', (e) => {
        if (document.activeElement && document.activeElement.tagName === 'INPUT') return;
        const bit = KEY_MAP[e.code];
        if (bit !== undefined) {
            joyBits &= ~bit;
            syncButtons();
        }
    });

    // On-screen buttons
    const TOUCH_MAP = {
        'dpad-up':    1 << 6,
        'dpad-down':  1 << 7,
        'dpad-left':  1 << 5,
        'dpad-right': 1 << 4,
        'btn-a':      1 << 0,
        'btn-b':      1 << 1,
        'btn-select': 1 << 2,
        'btn-start':  1 << 3,
    };

    for (const [id, bit] of Object.entries(TOUCH_MAP)) {
        const el = document.getElementById(id);
        if (!el) continue;
        el.addEventListener('pointerdown', (e) => {
            e.preventDefault();
            el.setPointerCapture(e.pointerId);
            touchBits |= bit;
            syncButtons();
        });
        el.addEventListener('pointerup', () => {
            touchBits &= ~bit;
            syncButtons();
        });
        el.addEventListener('pointercancel', () => {
            touchBits &= ~bit;
            syncButtons();
        });
    }

    // Gamepad polling via RAF
    function pollGamepad() {
        const gamepads = navigator.getGamepads ? navigator.getGamepads() : [];
        let bits = 0;
        for (const gp of gamepads) {
            if (!gp) continue;
            const B = gp.buttons;
            if (B[0]?.pressed)  bits |= 1 << 0;  // A
            if (B[1]?.pressed)  bits |= 1 << 1;  // B
            if (B[8]?.pressed)  bits |= 1 << 2;  // Select
            if (B[9]?.pressed)  bits |= 1 << 3;  // Start
            if (B[12]?.pressed) bits |= 1 << 6;  // Up
            if (B[13]?.pressed) bits |= 1 << 7;  // Down
            if (B[14]?.pressed) bits |= 1 << 5;  // Left
            if (B[15]?.pressed) bits |= 1 << 4;  // Right
            // Analog stick fallback
            if (gp.axes[0] >  0.5) bits |= 1 << 4;  // Right
            if (gp.axes[0] < -0.5) bits |= 1 << 5;  // Left
            if (gp.axes[1] >  0.5) bits |= 1 << 7;  // Down
            if (gp.axes[1] < -0.5) bits |= 1 << 6;  // Up
        }
        gamepadBits = bits;
        syncButtons();
        requestAnimationFrame(pollGamepad);
    }
    requestAnimationFrame(pollGamepad);

    // ── Sample ROM buttons ─────────────────────────────────────────────────────

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
