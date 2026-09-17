import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { createServer } from "node:http";
import { mkdir, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { dirname, resolve, posix } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium } from "../../web/node_modules/playwright-core/index.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
process.chdir(root);
const phase = process.argv[2];
assert.ok(["before", "after"].includes(phase), "Choose the before or after member of the single recorded-case matrix.");
assert.ok(process.env.DISPLAY, "Use the existing headful Xvfb configuration, not global graphics/sandbox changes.");
const shell = resolve(process.env.CLUBSCAPE_AUDIO_SHELL ?? "../m1-browser-shell");
const commit = "35e12584681fd45fa1671805f26adc64feae511a";
const codeRoot = resolve(shell, ".local/web-ui4-6f-bd4a693e");
const gameRoot = resolve(shell, ".local/source-ui4-6fdb60e4");
const work = resolve("tools/browser-audio-tests/.run-cue-timing");
const evidence = resolve("research/browser-audio-policy/cue-timing");
const reportArgument = process.argv.indexOf("--report-file");
const report = reportArgument === -1 ? resolve(evidence, `${phase}.json`) : resolve(process.argv[reportArgument + 1] ?? "");
assert.ok(report.startsWith(evidence + "/") && report.endsWith(".json"), "Use an owned cue-timing JSON report path.");
try {
  await stat(report);
  throw new Error("Refusing to overwrite timing evidence; choose a fresh --report-file path.");
} catch (error) {
  if (error.code !== "ENOENT") throw error;
}
const chrome = process.env.CLUBSCAPE_CHROME ??
  "/home/lramos15/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome";
const hash = (value) => createHash("sha256").update(value).digest("hex");
const gitBytes = (path) => execFileSync("git", ["show", `${commit}:${path}`], { maxBuffer: 16 * 1024 * 1024 });
const inputs = [], requests = [];
const version = execFileSync(chrome, ["--version"], { encoding: "utf8" }).trim();
assert.match(version, /Chrome for Testing 153\.0\.8010\.12/);
const playwrightVersion = JSON.parse(await readFile("web/node_modules/playwright-core/package.json", "utf8")).version;
const viteVersion = JSON.parse(await readFile(resolve(shell, "web/node_modules/vite/package.json"), "utf8")).version;
assert.equal(playwrightVersion, "1.63.0");
assert.equal(viteVersion, "8.3.0");
assert.equal(process.version, "v24.18.0");
await mkdir(work, { recursive: true });
await mkdir(evidence, { recursive: true });
const stage = resolve(work, "source");
const paths = execFileSync("git", ["ls-tree", "-r", "--name-only", commit, "--", "web/app", "web/audio", "web/shared"],
  { encoding: "utf8" }).trim().split("\n").filter((path) => path.endsWith(".ts") && !path.endsWith(".test.ts") &&
    (!path.includes("/tests/") || path.endsWith("/real-player-audio.ts") || path.endsWith("/player-audio-fixture.ts")));
for (const path of paths) {
  const bytes = phase === "after" && path.startsWith("web/audio/") ? await readFile(path) : gitBytes(path);
  const target = resolve(stage, path);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, bytes);
  inputs.push({ path, origin: phase === "after" && path.startsWith("web/audio/") ? "owned-candidate" : commit,
    sha256: hash(bytes), bytes: bytes.length });
}
if (phase === "after") {
  for (const name of await readdir("web/audio")) {
    const path = `web/audio/${name}`;
    if (!name.endsWith(".ts") || name.endsWith(".test.ts") || paths.includes(path)) continue;
    const bytes = await readFile(path);
    await writeFile(resolve(stage, path), bytes);
    inputs.push({ path, origin: "owned-candidate", sha256: hash(bytes), bytes: bytes.length });
  }
}
const delivery = JSON.parse(await readFile(resolve(codeRoot, "clubscape-web.json"), "utf8"));
const routes = new Map();
for (const entry of delivery.files) {
  const path = resolve(codeRoot, entry.path), bytes = await readFile(path);
  assert.equal(hash(bytes), entry.sha256, `Changed recorded shell delivery: ${entry.path}`);
  if (entry.url !== "/") routes.set(entry.url, { path, type: entry.content_type, sha256: entry.sha256 });
}
const originalBuild = JSON.parse(await readFile(resolve(codeRoot, "client/build.json"), "utf8"));
const contentPath = resolve(gameRoot, "content/manifest.json"), contentBytes = await readFile(contentPath);
assert.equal(hash(contentBytes), originalBuild.content.sha256);
const content = JSON.parse(contentBytes);
routes.set("/content/manifest.json", { path: contentPath, type: "application/json", sha256: hash(contentBytes) });
for (const asset of content.assets) {
  assert.ok((asset.url.startsWith("/assets/") || asset.url.startsWith("/content/")) && !asset.url.includes(".."));
  routes.set(asset.url, { path: resolve(gameRoot, `.${asset.url}`), type: asset.contentType, sha256: asset.sha256 });
}
if (phase === "after") {
  for (const [source, target] of [
    ["client/wasm/clubscape_wasm.js", "web/generated/protocol/clubscape_wasm.js"],
    ["client/wasm/clubscape_wasm_bg.wasm", "web/generated/protocol/clubscape_wasm_bg.wasm"],
  ]) {
    await mkdir(dirname(resolve(stage, target)), { recursive: true });
    await writeFile(resolve(stage, target), await readFile(resolve(codeRoot, source)));
  }
  const vite = await import(pathToFileURL(resolve(shell, "web/node_modules/vite/dist/node/index.js")));
  const output = resolve(work, "candidate");
  await vite.build({
    configFile: false, root: stage, publicDir: false, logLevel: "error", cacheDir: resolve(work, "vite-cache"),
    build: {
      target: "es2024", outDir: output, emptyOutDir: false, assetsDir: "client", assetsInlineLimit: 0,
      sourcemap: false, rolldownOptions: {
        preserveEntrySignatures: "strict", input: { bridge: resolve(stage, "web/app/bridge.ts") },
        output: { entryFileNames: "client/bridge.js" },
      },
    },
  });
  const outputs = [];
  async function register(directory, relative = "") {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const name = posix.join(relative, entry.name), path = resolve(directory, entry.name);
      if (entry.isDirectory()) { await register(path, name); continue; }
      const bytes = await readFile(path), sha256 = hash(bytes);
      outputs.push({ path: name, sha256, bytes: bytes.length });
      routes.set(`/${name}`, { path, sha256, type: name.endsWith(".wasm") ? "application/wasm" : "text/javascript" });
    }
  }
  await register(output);
  const artifact = Buffer.from(JSON.stringify({
    schemaVersion: 1, kind: "owned-audio-diagnostic-composition-not-production-ui-build",
    shellCommit: commit, sourcePackSha256: originalBuild.sourcePackSha256,
    benchmarkContractSha256: originalBuild.benchmarkContractSha256, inputs, outputs,
  }));
  const artifactPath = resolve(output, "client/build-artifact.json");
  await writeFile(artifactPath, artifact);
  routes.set("/client/build-artifact.json", { path: artifactPath, sha256: hash(artifact), type: "application/json" });
  const build = Buffer.from(JSON.stringify({
    ...originalBuild, buildId: `audio-cue-timing-${hash(artifact).slice(0, 16)}`,
    buildArtifactSha256: hash(artifact), components: { audio: true, ui: false, renderer: false },
  }));
  const buildPath = resolve(output, "client/build.json");
  await writeFile(buildPath, build);
  routes.set("/client/build.json", { path: buildPath, sha256: hash(build), type: "application/json" });
}
const fixture = await import(pathToFileURL(resolve(stage, "web/app/tests/real-player-audio.ts")));
const server = createServer(async (request, response) => {
  try {
    response.setHeader("cross-origin-opener-policy", "same-origin");
    response.setHeader("cross-origin-embedder-policy", "require-corp");
    response.setHeader("content-security-policy", "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:; media-src 'self' blob:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'");
    const url = new URL(request.url, "http://127.0.0.1");
    if (url.pathname === "/health") { response.writeHead(200).end("owned native timing fixture"); return; }
    if (url.pathname === "/fixture") {
      response.writeHead(200, { "content-type": "text/html" }).end(
        "<!doctype html><title>Original shell audio timing fixture - not game acceptance</title><body></body>");
      return;
    }
    if (url.pathname === "/favicon.ico") { response.writeHead(204).end(); return; }
    const entry = routes.get(url.pathname);
    if (!entry || url.search) { response.writeHead(404).end("Not an allowlisted source fixture route"); return; }
    const bytes = await readFile(entry.path);
    assert.equal(hash(bytes), entry.sha256, `Input changed during composed probe: ${url.pathname}`);
    requests.push({ url: url.pathname, bytes: bytes.length, sha256: entry.sha256 });
    response.writeHead(200, { "content-type": entry.type, "content-length": bytes.length, "cache-control": "no-store" });
    response.end(bytes);
  } catch (error) {
    response.writeHead(500).end(String(error));
  }
});
await new Promise((resolveReady) => server.listen(0, "127.0.0.1", resolveReady));
const origin = `http://127.0.0.1:${server.address().port}`;
assert.equal((await fetch(`${origin}/health`)).status, 200);
console.log(`Responsive owned composed timing fixture: ${origin}/health`);
const scratch = new Set(await readdir("web/audio"));
let context, page;
const result = { schemaVersion: 1, phase, shellCommit: commit, browser: version, startedAt: new Date().toISOString(),
  toolchain: { node: process.version, playwright: playwrightVersion, vite: viteVersion },
  originalShellBuild: originalBuild.buildId, inputs, requests, fixture: null, instrumentation: null,
  passed: false, failure: null, historicalEvidenceModified: false, gameJourney: false,
  browserOutputMuted: true, hostSpeakersOrMacOrM1Accepted: false };
try {
  context = await chromium.launchPersistentContext(resolve(work, "profile"), {
    executablePath: chrome, chromiumSandbox: true, headless: false,
    ignoreDefaultArgs: ["--enable-unsafe-swiftshader"],
    args: ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=vulkan",
      "--enable-gpu", "--ignore-gpu-blocklist", "--ozone-platform=x11", "--enable-automation", "--mute-audio"],
    viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1,
    env: { ...process.env, HOME: resolve(work, "home"), TMPDIR: resolve("web/audio"),
      XDG_CONFIG_HOME: resolve(work, "config"), XDG_CACHE_HOME: resolve(work, "cache") },
  });
  await context.route("**/*", (route) => route.request().url().startsWith(`${origin}/`) ? route.continue() : route.abort());
  page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  page.evaluate = async (fn, arg) => {
    const reply = await cdp.send("Runtime.evaluate", {
      expression: `(${fn.toString()})(${JSON.stringify(arg) ?? ""})`, userGesture: false,
      awaitPromise: true, returnByValue: true,
    });
    if (reply.exceptionDetails) throw new Error(reply.exceptionDetails.exception?.description ?? reply.exceptionDetails.text);
    return reply.result.value;
  };
  const command = (await cdp.send("Browser.getBrowserCommandLine")).arguments;
  assert.equal(command.includes("--no-sandbox"), false);
  assert.equal(command.includes("--mute-audio"), true);
  await page.goto(origin + "/fixture");
  assert.equal(await page.evaluate(() => crossOriginIsolated), true);
  await page.evaluate(() => {
    const NativeContext = AudioContext, interval = window.setInterval, connect = AudioNode.prototype.connect;
    const observation = { contexts: [], starts: [], ends: [], ticks: [], connections: [], edges: new Map(), buffers: new Map() };
    window.__cueTiming = observation;
    globalThis.AudioContext = class extends NativeContext {
      constructor(options) {
        super(options);
        observation.contexts.push(this);
        this.__options = options;
      }
      createBufferSource() {
        const source = super.createBufferSource(), original = source.start.bind(source), context = this;
        const id = observation.starts.length + 1;
        source.start = (...args) => {
          const beforeCallTime = context.currentTime;
          original(...args);
          observation.buffers.set(source.buffer.length, source.buffer);
          let node = source, gain = 1;
          const seen = new Set();
          while (node && !seen.has(node)) {
            seen.add(node);
            if (node instanceof GainNode) gain *= node.gain.value;
            node = observation.edges.get(node);
          }
          observation.starts.push({ id, frames: source.buffer.length, sampleRate: source.buffer.sampleRate,
            state: context.state, beforeCallTime, audioTime: context.currentTime, wallTime: performance.now(),
            when: args[0] ?? 0, offset: args[1] ?? 0, duration: args[2] ?? null, gain });
        };
        source.addEventListener("ended", () => observation.ends.push({ id, audioTime: context.currentTime, wallTime: performance.now() }));
        return source;
      }
    };
    AudioNode.prototype.connect = function (...args) {
      const result = connect.apply(this, args);
      observation.edges.set(this, args[0]);
      if (args[0] === this.context.destination) observation.connections.push({ context: this.context, source: this });
      return result;
    };
    window.setInterval = (callback, delay, ...args) => interval(() => {
      const context = observation.contexts.at(-1);
      if (delay === 5 && context) {
        const before = { audioTime: context.currentTime, wallTime: performance.now(), state: context.state };
        callback(...args);
        observation.ticks.push({ ...before, afterTime: context.currentTime, afterWall: performance.now() });
      } else callback(...args);
    }, delay);
  });
  result.fixture = await fixture.checkPlayerAudioPreferences(page);
  result.passed = result.fixture.nativeCueTiming.passed;
  result.instrumentation = await page.evaluate(async () => {
    const observed = window.__cueTiming;
    return {
      contexts: observed.contexts.map((context) => ({ options: context.__options, sampleRate: context.sampleRate,
        state: context.state, currentTime: context.currentTime, baseLatency: context.baseLatency, outputLatency: context.outputLatency })),
      starts: observed.starts, ends: observed.ends, ticks: observed.ticks,
      productionDestinationConnections: observed.connections.length,
      addedLiveMonitoringNodes: 0,
      crossOriginIsolated,
    };
  });
  const originalManifest = JSON.parse(await readFile("assets/manifests/osrs/audio-runtime.json", "utf8"));
  const originalCue = originalManifest.assets.find((asset) => asset.kind === "sfx" && asset.source_group === 2266);
  result.pcm = await page.evaluate(async ({ frames, firstNonzeroFrame, dispatches }) => {
    const observed = window.__cueTiming;
    const buffer = observed.buffers.get(frames);
    if (!buffer) throw new Error("The actual composed path did not decode/start the original control cue.");
    const comparisons = [];
    for (const dispatch of dispatches) {
      const actual = observed.starts.find((start) => start.frames === frames && start.when === dispatch.when);
      if (!actual) throw new Error("A dispatch trace does not correspond to a real AudioBufferSourceNode.start.");
      const origin = dispatch.requestedAt ?? dispatch.dueAt - 0.02;
      const effectiveWhen = Math.max(dispatch.when, actual.beforeCallTime);
      const lead = effectiveWhen - origin;
      const offline = new OfflineAudioContext(buffer.numberOfChannels, Math.ceil((lead + buffer.duration + 0.02) * 22050), 22050);
      const source = offline.createBufferSource(), gain = offline.createGain();
      source.buffer = buffer; gain.gain.value = actual.gain;
      source.connect(gain); gain.connect(offline.destination); source.start(lead);
      const rendered = await offline.startRendering();
      const pcm = rendered.getChannelData(0), input = buffer.getChannelData(0);
      const onset = pcm.findIndex((value) => value !== 0);
      const expected = Math.round(lead * 22050) + firstNonzeroFrame;
      let maximumError = 0, clips = 0;
      for (let i = 0; i < pcm.length; i++) {
        if (Math.abs(pcm[i]) > 1) clips++;
        const sourceFrame = i - Math.round(lead * 22050);
        const expectedSample = sourceFrame >= 0 && sourceFrame < input.length ? input[sourceFrame] * actual.gain : 0;
        maximumError = Math.max(maximumError, Math.abs(pcm[i] - expectedSample));
      }
      comparisons.push({ eventId: dispatch.eventId, requestedAt: dispatch.requestedAt ?? null,
        nativeStartCall: actual.audioTime, scheduledWhen: actual.when, actualSourceGain: actual.gain,
        effectiveWhen, actualDispatchLateMs: Math.max(0, effectiveWhen - dispatch.dueAt) * 1000,
        pcmOnsetFrame: onset, expectedOnsetFrame: expected, onsetErrorSamples: Math.abs(onset - expected),
        maximumError, addedClipSamples: clips,
        absoluteSourceCycleErrorMs: dispatch.requestedAt === undefined ? null : Math.abs(lead - 0.02) * 1000 });
      source.disconnect(); gain.disconnect();
    }
    return { rate: 22050, originalFrames: frames, firstNonzeroFrame, liveMonitoringNodesAdded: 0,
      scope: "Actual enabled native start calls and their unchanged decoded PCM rendered independently after the live context closes.",
      comparisons };
  }, { frames: originalCue.signal.frames, firstNonzeroFrame: originalCue.signal.first_nonzero_frame,
    dispatches: result.fixture.nativeCueTiming.dispatches });
  for (const comparison of result.pcm.comparisons) {
    assert.ok(comparison.onsetErrorSamples <= 1);
    assert.ok(comparison.maximumError <= 0.000023);
    assert.equal(comparison.addedClipSamples, 0);
    if (phase === "after") assert.ok(comparison.absoluteSourceCycleErrorMs <= result.fixture.nativeCueTiming.dispatchToleranceMs,
      `An actual onset was moved outside its independent one-source-cycle bound: ${JSON.stringify(comparison)}`);
    assert.ok(comparison.actualDispatchLateMs <= result.fixture.nativeCueTiming.dispatchToleranceMs,
      "A real source start was late even though its requested time looked on time.");
  }
  assert.equal(result.fixture.checks.length, 6);
  assert.equal(result.fixture.nativeCueTiming.comparison.length, 2);
  if (!result.passed) throw new Error(`Unchanged composed native gate failed: ${result.fixture.nativeCueTiming.dispatchOverruns} overruns.`);
} catch (error) {
  result.passed = false;
  result.failure = String(error.stack ?? error);
  process.exitCode = 1;
} finally {
  await context?.close();
  server.closeAllConnections();
  await new Promise((resolveClosed) => server.close(resolveClosed));
  result.finishedAt = new Date().toISOString();
  await mkdir(dirname(report), { recursive: true });
  await writeFile(report, JSON.stringify(result, null, 2) + "\n");
  await rm(work, { recursive: true, force: true });
  for (const name of await readdir("web/audio")) {
    if (!scratch.has(name) && /^(?:\.?org\.chromium\.|playwright-)/.test(name))
      await rm(resolve("web/audio", name), { recursive: true, force: true });
  }
}
const timing = result.fixture?.nativeCueTiming;
console.log(JSON.stringify({ phase, passed: result.passed, checks: result.fixture?.checks.length,
  dispatches: timing?.dispatches.length, overruns: timing?.dispatchOverruns,
  maxLateMs: timing ? Math.max(...timing.dispatches.map((entry) => entry.lateMs)) : null,
  report, failure: result.failure }));
