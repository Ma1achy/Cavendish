// Starts Cavendish in the page (index.html). WebGPU only (docs/research/web-build-and-pages.md §2):
// a browser without it, or with it but no graphics adapter, is told so plainly, with the browsers
// that work, before the 2 MB module is fetched at all. Otherwise the module is compiled once, kept
// for the forward stage's workers (globalThis.cavendishWasm, read by src/platform/web.rs), and run.

const message = document.getElementById("message");

const WORKING =
  "Open this page in a current Chrome or Edge (Windows, macOS, ChromeOS, Android), Safari 26 or later, " +
  "or Firefox 141 or later on Windows. On Linux, Chrome may need WebGPU switched on in " +
  "<code>chrome://flags</code>. The native application is in the " +
  "<a href=\"https://github.com/Ma1achy/Cavendish\">repository</a>.";

function refuse(said) {
  document.getElementById("cavendish")?.remove();
  message.innerHTML = `<p>${said} ${WORKING}</p>`;
}

async function start() {
  if (!("gpu" in navigator)) {
    refuse("<strong>This browser does not offer WebGPU</strong>, which Cavendish draws and computes with.");
    return;
  }
  let adapter = null;
  try {
    adapter = await navigator.gpu.requestAdapter();
  } catch (error) {
    console.warn("WebGPU's adapter request failed:", error);
  }
  if (!adapter) {
    refuse("<strong>This browser offers WebGPU but found no graphics adapter for it</strong>, and Cavendish " +
      "draws and computes with WebGPU: it may be switched off in the browser's settings, or the graphics " +
      "driver blocked.");
    return;
  }
  const wasm = await WebAssembly.compileStreaming(fetch("./cavendish_bg.wasm"));
  globalThis.cavendishWasm = wasm;
  const module = await import("./cavendish.js");
  await module.default({ module_or_path: wasm });
}

start().catch((error) => {
  console.error(error);
  message.textContent = "Cavendish could not start: " + error;
});
