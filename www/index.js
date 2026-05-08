import init, { run, load_rom, step_frame, get_framebuffer } from "../pkg/gpuboy_wasm.js";

let animationId = null;

function startLoop() {
    const canvas = document.getElementById("screen");
    const ctx = canvas.getContext("2d");

    function tick() {
        step_frame();
        const fb = get_framebuffer();
        const imageData = new ImageData(new Uint8ClampedArray(fb), 160, 144);
        ctx.putImageData(imageData, 0, 0);
        animationId = requestAnimationFrame(tick);
    }

    animationId = requestAnimationFrame(tick);
}

async function main() {
    await init();
    run();

    document.getElementById("rom-picker").addEventListener("change", (e) => {
        const file = e.target.files[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onerror = (ev) => console.log("FileReader error: " + ev.target.error);
        reader.onload = (ev) => {
            if (ev.target.result instanceof ArrayBuffer) {
                load_rom(new Uint8Array(ev.target.result));
                if (animationId !== null) {
                    cancelAnimationFrame(animationId);
                    animationId = null;
                }
                startLoop();
            }
        };
        reader.readAsArrayBuffer(file);
    });
}

main().catch((err) => {
    const el = document.getElementById("error");
    if (el) {
        el.textContent = `Failed to load: ${err}`;
        el.style.display = "block";
    }
});
