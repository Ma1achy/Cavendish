// Takes a screenshot of the web build in headless Chrome with WebGPU, with no visible window, and
// prints the page's console. Used by `cargo xtask demo S17` and by hand:
//
//   node scripts/web_screenshot.mjs <chrome.exe> <url> <out.png> [seconds=12] [1440x900] [no-webgpu]
//
// With CAV_DOWNLOADS set to a folder, files the page downloads are saved there. With CAV_CLICK set
// to an element's id, that element is clicked as a person would (a user gesture) each time it
// appears: files an address's `#exec` makes wait behind such a button (`held-downloads`).
//
// With CAV_FRAMES set, every frame the page draws is timed (requestAnimationFrame) and, at the
// end, the frames after the application started are summarised: how many, the longest, and how
// many took longer than CAV_FRAMES milliseconds (the budget; 50 if it is not a number). A long
// frame is the page's thread held: by the engine's work, a tree, or the drawing.
//
// With `no-webgpu` the page is loaded as a browser without WebGPU sees it (`navigator.gpu` removed
// before the page's script runs): it must show its message and never fetch the module; exit 0 then
// means exactly that. `no-adapter` is the same for a browser with WebGPU but no adapter
// (`requestAdapter` answers null).
//
// The browser's own log is watched too (the DevTools `Log` domain): a Content-Security-Policy
// violation, a failed load or any other error there fails the run as a console error does.
//
// Chrome is started here with a throw-away profile under the output's folder, talked to over the
// DevTools protocol (Node's own WebSocket), and closed here: nothing is left running. Exit status 1
// when the page logs a panic or an error, or no picture could be taken.

import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";

const [chrome, url, out, seconds = "12", size = "1440x900", mode = ""] = process.argv.slice(2);
const withoutGpu = mode === "no-webgpu";
const withoutAdapter = mode === "no-adapter";
if (!chrome || !url || !out) {
  console.error("usage: node web_screenshot.mjs <chrome> <url> <out.png> [seconds] [WxH]");
  process.exit(2);
}
const [width, height] = size.split("x").map(Number);
const profile = mkdtempSync(join(dirname(resolve(out)), "chrome-profile-"));
const browser = spawn(chrome, [
  "--headless=new",
  "--enable-unsafe-webgpu",
  "--no-first-run",
  "--no-default-browser-check",
  "--disable-component-update", // no component downloads (they land in CAV_DOWNLOADS) and no traffic
  "--disable-background-networking",
  // A fresh profile otherwise fetches Chrome's on-device model (tens of MB) into the downloads.
  "--disable-features=OptimizationGuideModelDownloading,OptimizationHintsFetching,OptimizationTargetPrediction,OptimizationHints",
  "--remote-debugging-port=0",
  `--user-data-dir=${profile}`,
  `--window-size=${width},${height}`,
  "about:blank",
], { stdio: ["ignore", "ignore", "pipe"] });

let failed = false;
const finish = (code) => {
  browser.kill();
  browser.once("exit", () => {
    try { rmSync(profile, { recursive: true, force: true, maxRetries: 20, retryDelay: 250 }); } catch { /* a locked file: left */ }
    process.exit(code);
  });
};
const deadline = setTimeout(() => { console.error("timed out"); finish(1); }, (Number(seconds) + 60) * 1000);

const endpoint = await new Promise((done, fail) => {
  let text = "";
  browser.stderr.on("data", (chunk) => {
    text += chunk;
    const found = text.match(/DevTools listening on (ws:\/\/\S+)/);
    if (found) done(found[1]);
  });
  browser.once("exit", () => fail(new Error("chrome exited before it listened")));
});

const socket = new WebSocket(endpoint);
await new Promise((done) => socket.addEventListener("open", done, { once: true }));
let next = 0;
const waiting = new Map();
const listeners = [];
socket.addEventListener("message", (event) => {
  const message = JSON.parse(event.data);
  if (message.id !== undefined && waiting.has(message.id)) {
    waiting.get(message.id)(message);
    waiting.delete(message.id);
  } else {
    for (const listen of listeners) listen(message);
  }
});
const send = (method, params = {}, sessionId) => new Promise((done) => {
  const id = ++next;
  waiting.set(id, done);
  socket.send(JSON.stringify({ id, method, params, sessionId }));
});

if (process.env.CAV_DOWNLOADS) {
  await send("Browser.setDownloadBehavior", { behavior: "allow", downloadPath: resolve(process.env.CAV_DOWNLOADS) });
}
const { result: { targetId } } = await send("Target.createTarget", { url: "about:blank" });
const { result: { sessionId } } = await send("Target.attachToTarget", { targetId, flatten: true });
listeners.push((message) => {
  if (message.sessionId !== sessionId) return;
  if (message.method === "Runtime.consoleAPICalled") {
    const text = message.params.args.map((a) => a.value ?? a.description ?? "").join(" ");
    console.log(`console.${message.params.type}: ${text}`);
    if (message.params.type === "error" || /panicked/.test(text)) failed = true;
  }
  if (message.method === "Log.entryAdded") {
    const entry = message.params.entry;
    console.log(`log.${entry.level} (${entry.source}): ${entry.text}${entry.url ? " " + entry.url : ""}`);
    // A favicon the page does not have is the browser's guess, not the page's fault.
    if (entry.level === "error" && !/favicon\.ico/.test(entry.url ?? "")) failed = true;
  }
  if (message.method === "Runtime.exceptionThrown") {
    console.log(`exception: ${message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text}`);
    failed = true;
  }
});
await send("Runtime.enable", {}, sessionId);
await send("Log.enable", {}, sessionId);
if (withoutGpu || withoutAdapter) {
  await send("Page.enable", {}, sessionId);
  const source = withoutGpu ? "delete Navigator.prototype.gpu;" : "GPU.prototype.requestAdapter = async () => null;";
  await send("Page.addScriptToEvaluateOnNewDocument", { source }, sessionId);
}
const budget = Number(process.env.CAV_FRAMES) || 50;
if (process.env.CAV_FRAMES) {
  await send("Page.enable", {}, sessionId);
  await send("Page.addScriptToEvaluateOnNewDocument", { source: `
    window.__frames = []; window.__runningAt = null;
    let last = performance.now();
    const tick = (now) => {
      if (window.__runningAt === null && document.readyState === "complete" && !document.getElementById("message")) {
        window.__runningAt = now;
      }
      window.__frames.push([last, now - last]);
      last = now;
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);` }, sessionId);
}
await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false }, sessionId);
const started = Date.now();
await send("Page.navigate", { url }, sessionId);
// The application is running once it has removed the page's own message ("Loading Cavendish…").
let running = null;
let clicked = 0;
const click = process.env.CAV_CLICK;
while (Date.now() - started < Number(seconds) * 1000) {
  if (running === null) {
    const gone = await send("Runtime.evaluate", {
      expression: "document.readyState === 'complete' && document.getElementById('message') === null",
      returnByValue: true,
    }, sessionId);
    if (gone.result?.result?.value === true) running = (Date.now() - started) / 1000;
  }
  if (click) {
    const found = await send("Runtime.evaluate", {
      expression: `(() => { const e = document.getElementById(${JSON.stringify(click)}); if (!e) return null; const said = e.textContent; e.click(); return said; })()`,
      returnByValue: true,
      userGesture: true,
    }, sessionId);
    const said = found.result?.result?.value;
    if (typeof said === "string") {
      clicked += 1;
      console.log(`clicked #${click} at ${((Date.now() - started) / 1000).toFixed(2)} s: ${said}`);
    }
  }
  await new Promise((done) => setTimeout(done, 50));
}
console.log(running === null ? "the application did not start" : `running ${running.toFixed(2)} s after the address was opened`);
if (click && clicked === 0) {
  console.log(`#${click} never appeared`);
  failed = true;
}
if (withoutGpu || withoutAdapter) {
  const said = await send("Runtime.evaluate", {
    expression: "document.getElementById('message')?.textContent ?? ''",
    returnByValue: true,
  }, sessionId);
  const fetched = await send("Runtime.evaluate", {
    expression: "performance.getEntriesByType('resource').some(r => r.name.endsWith('.wasm'))",
    returnByValue: true,
  }, sessionId);
  console.log(`message: ${said.result?.result?.value}`);
  const wanted = withoutGpu ? /does not offer WebGPU/ : /found no graphics adapter/;
  failed = failed || !wanted.test(said.result?.result?.value ?? "") || fetched.result?.result?.value !== false;
} else if (running === null) {
  failed = true;
}
if (process.env.CAV_FRAMES) {
  const summary = await send("Runtime.evaluate", {
    expression: `(() => {
      const from = window.__runningAt ?? Infinity;
      const after = window.__frames.filter(([start]) => start >= from).map(([start, ms]) => [Math.round(start - from), Math.round(ms)]);
      const long = after.filter(([, ms]) => ms > ${budget});
      const worst = [...after].sort((a, b) => b[1] - a[1]).slice(0, 5);
      return JSON.stringify({ frames: after.length, longest: worst[0]?.[1] ?? null, over: long.length, worst });
    })()`,
    returnByValue: true,
  }, sessionId);
  const frames = JSON.parse(summary.result?.result?.value ?? "{}");
  console.log(`frames after start: ${frames.frames}, longest ${frames.longest} ms, ${frames.over} over ${budget} ms; worst (ms after start, ms): ${JSON.stringify(frames.worst)}`);
}
const shot = await send("Page.captureScreenshot", { format: "png" }, sessionId);
const loaded = await send("Runtime.evaluate", {
  expression: "JSON.stringify(performance.getEntriesByType('resource').map(r => [r.name.split('/').pop(), Math.round(r.duration), r.transferSize]))",
  returnByValue: true,
}, sessionId);
console.log(`resources (name, ms, bytes): ${loaded.result?.result?.value}`);
console.log(`waited ${((Date.now() - started) / 1000).toFixed(1)} s`);
clearTimeout(deadline);
if (!shot.result?.data) {
  console.error("no screenshot");
  finish(1);
} else {
  writeFileSync(out, Buffer.from(shot.result.data, "base64"));
  console.log(`${basename(out)}: written`);
  finish(failed ? 1 : 0);
}
