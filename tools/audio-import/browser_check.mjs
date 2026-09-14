import { createServer } from "node:http";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { execFileSync, spawn } from "node:child_process";
import { resolve, join, relative } from "node:path";
import { createHash } from "node:crypto";

const root = process.cwd();
const chrome = process.argv[2];
if (!chrome) throw new Error("Usage: node tools/audio-import/browser_check.mjs /path/to/verified/chrome");
const workspace = resolve(".local/audio-import/browser-check");
const socketWork = resolve(".local");
await mkdir(workspace, { recursive: true });
await mkdir(socketWork, { recursive: true });
for (const name of ["profile", "home", "cache", "config", "work"]) {
  await mkdir(join(workspace, name), { recursive: true });
}
const manifestPath = "assets/manifests/osrs/audio-runtime.json";
const manifestBytes = await readFile(manifestPath);
const manifest = JSON.parse(manifestBytes);
execFileSync("python3", ["tools/audio-import/browser_oracles.py"], { stdio: "inherit", timeout: 180000 });
const oracleBytes = await readFile(".local/audio-import/browser-oracles.json");
const oracleRecord = JSON.parse(oracleBytes);
if (oracleRecord.manifest.sha256 !== createHash("sha256").update(manifestBytes).digest("hex")) {
  throw new Error("Decoder oracle manifest changed");
}
const allowed = new Map();
for (const record of manifest.assets) {
  const file = resolve(record.path);
  if (!record.path.startsWith("assets/source/osrs/audio-runtime/") || relative(root, file).startsWith("..")) {
    throw new Error("Refusing to serve a non-audio asset");
  }
  allowed.set(`/${record.path}`, file);
}
const html = `<!doctype html><meta charset="utf-8"><title>Offline source-audio decoder test</title>
<p>This verifies source-file decoding only. It never starts audible playback.</p>
<script>
window.checkAudioFiles = async () => {
  const manifest = await (await fetch('/manifest.json')).json();
  const oracles = await (await fetch('/decoder-oracles.json')).json();
  const context = new OfflineAudioContext(2, 1, 22050);
  const records = [];
  for (const asset of manifest.assets) {
    const response = await fetch('/' + asset.path);
    if (!response.ok) throw new Error('Missing source audio: ' + asset.path);
    const buffer = await context.decodeAudioData(await response.arrayBuffer());
    if (buffer.sampleRate !== 22050 || buffer.length !== asset.signal.frames ||
        buffer.numberOfChannels !== asset.signal.channels) throw new Error('Decoded format mismatch: ' + asset.path);
    const hashes = [];
    let peak = 0, nonzero = 0;
    for (let channel = 0; channel < buffer.numberOfChannels; channel++) {
      const data = buffer.getChannelData(channel);
      const digest = await crypto.subtle.digest('SHA-256', data);
      hashes.push(Array.from(new Uint8Array(digest), n => n.toString(16).padStart(2, '0')).join(''));
      for (const value of data) {
        if (!Number.isFinite(value)) throw new Error('Invalid sample');
        peak = Math.max(peak, Math.abs(value));
        if (value !== 0) nonzero++;
      }
    }
    if (!nonzero || peak >= 1) throw new Error('Silent/full-scale decode: ' + asset.path);
    const model = oracles.models[asset.asset_id].find(model =>
      JSON.stringify(model.float32_channel_sha256) === JSON.stringify(hashes));
    if (!model) throw new Error('Browser PCM has no exact verified integer-to-float representation: ' +
      JSON.stringify({path: asset.path, actual: hashes, expected: oracles.models[asset.asset_id]}));
    records.push({asset_id: asset.asset_id, sample_rate: buffer.sampleRate, frames: buffer.length,
      channels: buffer.numberOfChannels, peak, float32_channel_sha256: hashes,
      exact_integer_to_float_model: model});
  }
  return {count: records.length, records};
};
</script>`;
const server = createServer(async (request, response) => {
  try {
    const path = decodeURIComponent(new URL(request.url, "http://127.0.0.1").pathname);
    let bytes, type;
    if (path === "/") {
      bytes = Buffer.from(html);
      type = "text/html; charset=utf-8";
    } else if (path === "/manifest.json") {
      bytes = manifestBytes;
      type = "application/json";
    } else if (path === "/decoder-oracles.json") {
      bytes = oracleBytes;
      type = "application/json";
    } else if (allowed.has(path)) {
      bytes = await readFile(allowed.get(path));
      type = "audio/flac";
    } else {
      response.writeHead(404).end();
      return;
    }
    response.writeHead(200, { "Content-Type": type, "Content-Length": bytes.length, "Cache-Control": "no-store" });
    response.end(bytes);
  } catch {
    response.writeHead(500).end();
  }
});
await new Promise((done, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", done);
});
const origin = `http://127.0.0.1:${server.address().port}`;
if (!(await fetch(`${origin}/manifest.json`)).ok) throw new Error("Decoder fixture server did not respond");
let browser, socket;
const pending = new Map();
const eventWaiters = [];
let nextId = 0;
function send(method, params = {}, sessionId) {
  return new Promise((done, reject) => {
    const id = ++nextId;
    const timeout = setTimeout(() => { pending.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, 180000);
    pending.set(id, { done, reject, timeout });
    socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
  });
}
function event(method, sessionId) {
  return new Promise((done, reject) => {
    const waiter = { method, sessionId, done, reject };
    waiter.timeout = setTimeout(() => reject(new Error(`CDP event timeout: ${method}`)), 30000);
    eventWaiters.push(waiter);
  });
}
try {
  browser = spawn(chrome, [
    "--headless=new", "--disable-gpu", "--no-first-run", "--no-default-browser-check",
    "--disable-background-networking", "--disable-component-update", "--disable-sync",
    "--disable-dev-shm-usage", "--remote-debugging-address=127.0.0.1", "--remote-debugging-port=0",
    `--user-data-dir=${join(workspace, "profile")}`, "about:blank",
  ], {
    stdio: ["ignore", "ignore", "pipe"],
    env: {
      ...process.env, HOME: join(workspace, "home"), TMPDIR: socketWork,
      XDG_CACHE_HOME: join(workspace, "cache"), XDG_CONFIG_HOME: join(workspace, "config"),
    },
  });
  const debuggerUrl = await new Promise((done, reject) => {
    let stderr = "";
    const timer = setTimeout(() => reject(new Error(`Chrome startup timed out: ${stderr.slice(-1500)}`)), 30000);
    browser.once("error", reject);
    browser.once("exit", (code, signal) => {
      clearTimeout(timer);
      reject(new Error(`Chrome exited during startup: ${code}/${signal}: ${stderr.slice(-4000)}`));
    });
    browser.stderr.on("data", chunk => {
      stderr = (stderr + chunk.toString()).slice(-8000);
      const match = stderr.match(/DevTools listening on (\S+)/);
      if (match) { clearTimeout(timer); done(match[1]); }
    });
  });
  socket = new WebSocket(debuggerUrl);
  await new Promise((done, reject) => {
    socket.addEventListener("open", done, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  socket.addEventListener("message", message => {
    const value = JSON.parse(message.data);
    if (value.id && pending.has(value.id)) {
      const request = pending.get(value.id);
      pending.delete(value.id);
      clearTimeout(request.timeout);
      if (value.error) request.reject(new Error(JSON.stringify(value.error)));
      else request.done(value.result);
    }
    for (let i = eventWaiters.length - 1; i >= 0; i--) {
      const waiter = eventWaiters[i];
      if (value.method === waiter.method && value.sessionId === waiter.sessionId) {
        clearTimeout(waiter.timeout);
        eventWaiters.splice(i, 1);
        waiter.done(value.params);
      }
    }
  });
  const version = await send("Browser.getVersion");
  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
  await send("Page.enable", {}, sessionId);
  const loaded = event("Page.loadEventFired", sessionId);
  await send("Page.navigate", { url: origin }, sessionId);
  await loaded;
  const result = await send("Runtime.evaluate", { expression: "checkAudioFiles()", awaitPromise: true, returnByValue: true }, sessionId);
  if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
  const decoded = result.result.value;
  if (decoded.count !== manifest.assets.length) throw new Error("Incomplete browser decode result");
  const report = {
    schema_version: 1,
    scope: "OfflineAudioContext.decodeAudioData source-file check only; no audible playback, UI/game presentation, autoplay, region events or owner acceptance.",
    result: "passed", manifest_sha256: createHash("sha256").update(manifestBytes).digest("hex"),
    browser: version, sandbox_not_disabled: true,
    original_pcm_representation_verified: true, source_recording: false, runtime_browser_audio_accepted: false,
    conversion_note: "Every browser float channel exactly matches an independently computed integer-to-float codec model. Chromium's positive int16 samples use a float32 reciprocal of32767 rather than32768; this numerical representation difference is recorded, not hidden behind an error tolerance or claimed as presentation acceptance.",
    ...decoded,
  };
  await writeFile("research/audio-source/browser-decode.json", JSON.stringify(report, null, 2) + "\n");
  console.log(`Browser decoded ${decoded.count} source files with exact source-PCM representation, expected durations and no full-scale samples (${version.product}).`);
} finally {
  if (socket?.readyState === WebSocket.OPEN) {
    try {
      await Promise.race([send("Browser.close"), new Promise(done => setTimeout(done, 3000))]);
    } catch { /* The process may close before replying. */ }
    socket.close();
  }
  if (browser && browser.exitCode === null && browser.signalCode === null) {
    await new Promise(done => {
      const timer = setTimeout(() => { browser.kill("SIGTERM"); done(); }, 5000);
      browser.once("exit", () => { clearTimeout(timer); done(); });
    });
  }
  for (const request of pending.values()) clearTimeout(request.timeout);
  for (const waiter of eventWaiters) clearTimeout(waiter.timeout);
  await new Promise(done => server.close(done));
}
