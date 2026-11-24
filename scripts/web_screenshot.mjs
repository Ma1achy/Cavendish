// Takes a screenshot of the web build in headless Chrome with WebGPU, with no visible window, and
// prints the page's console. Used by `cargo xtask demo S17` and by hand:
//
//   node scripts/web_screenshot.mjs <chrome.exe> <url> <out.png> [seconds=12] [1440x900]
//
// Chrome is started here with a throw-away profile under the output's folder, talked to over the
// DevTools protocol (Node's own WebSocket), and closed here: nothing is left running. Exit status 1
// when the page logs a panic or an error, or no picture could be taken.

import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";

const [chrome, url, out, seconds = "12", size = "1440x900"] = process.argv.slice(2);
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
  "--remote-debugging-port=0",
  `--user-data-dir=${profile}`,
  `--window-size=${width},${height}`,
  "about:blank",
], { stdio: ["ignore", "ignore", "pipe"] });

let failed = false;
const finish = (code) => {
  browser.kill();
  browser.once("exit", () => {
    try { rmSync(profile, { recursive: true, force: true }); } catch { /* a locked file: left */ }
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

const { result: { targetId } } = await send("Target.createTarget", { url: "about:blank" });
const { result: { sessionId } } = await send("Target.attachToTarget", { targetId, flatten: true });
listeners.push((message) => {
  if (message.sessionId !== sessionId) return;
  if (message.method === "Runtime.consoleAPICalled") {
    const text = message.params.args.map((a) => a.value ?? a.description ?? "").join(" ");
    console.log(`console.${message.params.type}: ${text}`);
    if (message.params.type === "error" || /panicked/.test(text)) failed = true;
  }
  if (message.method === "Runtime.exceptionThrown") {
    console.log(`exception: ${message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text}`);
    failed = true;
  }
});
await send("Runtime.enable", {}, sessionId);
await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false }, sessionId);
const started = Date.now();
await send("Page.navigate", { url }, sessionId);
// The application is running once it has removed the page's own message ("Loading Cavendish…").
let running = null;
while (Date.now() - started < Number(seconds) * 1000) {
  if (running === null) {
    const gone = await send("Runtime.evaluate", {
      expression: "document.readyState === 'complete' && document.getElementById('message') === null",
      returnByValue: true,
    }, sessionId);
    if (gone.result?.result?.value === true) running = (Date.now() - started) / 1000;
  }
  await new Promise((done) => setTimeout(done, 50));
}
console.log(running === null ? "the application did not start" : `running ${running.toFixed(2)} s after the address was opened`);
if (running === null) failed = true;
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
