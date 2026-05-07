import init, { run, load_rom } from "../pkg/gpuboy_wasm.js";

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
