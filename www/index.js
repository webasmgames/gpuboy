import init, { run } from "../pkg/gpuboy_wasm.js";

async function main() {
    await init();
    run();
}

main().catch((err) => {
    const el = document.getElementById("error");
    if (el) {
        el.textContent = `Failed to load: ${err}`;
        el.style.display = "block";
    }
});
