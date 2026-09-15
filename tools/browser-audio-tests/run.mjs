import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { createServer } from "node:http";
import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { stripTypeScriptTypes } from "node:module";
import { fileURLToPath } from "node:url";
import { chromium } from "../../web/node_modules/playwright-core/index.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
process.chdir(root);
const owned = "tools/browser-audio-tests";
const run = `${owned}/.run`;
const chrome = process.env.CLUBSCAPE_CHROME ??
  "/home/lramos15/.cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome";
const version = execFileSync(chrome, ["--version"], { encoding: "utf8" }).trim();
assert.match(version, /Chrome for Testing 153\./, "Use the verified sandboxed Chrome 153, not an unrecorded browser.");
const manifest = JSON.parse(await readFile("assets/manifests/osrs/audio-runtime.json", "utf8"));
const reference = JSON.parse(await readFile("research/reference-pack/v1/audio-reference.json", "utf8"));
const sources = [...manifest.assets, ...reference.reference_templates];
const registry = new Map(sources.map((a) => [a.asset_id ?? a.id, a.path]));
for (const path of [
  "assets/manifests/osrs/audio-runtime.json", "research/audio-source/source-map.json",
  "research/reference-pack/v1/audio-reference.json",
]) registry.set(path, path);
const requests = [];
const faults = new Map();
const modules = new Map();
const implementationFiles = [];
for (const name of await readdir("web/audio")) {
  if (name.endsWith(".ts") && !name.endsWith(".test.ts")) {
    const path = `web/audio/${name}`;
    const source = await readFile(path, "utf8");
    implementationFiles.push({ path, sha256: createHash("sha256").update(source).digest("hex") });
    modules.set(`/${path}`, stripTypeScriptTypes(source));
  }
}
modules.set("/web/shared/contracts.ts", stripTypeScriptTypes(await readFile("web/shared/contracts.ts", "utf8")));
const staticFiles = new Map([
  ["/", "fixture.html"], ["/fixture.mjs", "fixture.mjs"], ["/monitor.js", "monitor.js"],
]);
const digest = (data) => createHash("sha256").update(data).digest("hex");
const originalsBefore = new Map();
for (const asset of sources) {
  const hash = digest(await readFile(asset.path));
  assert.equal(hash, asset.sha256, `Pinned original changed before testing: ${asset.path}`);
  originalsBefore.set(asset.path, hash);
}
for (const path of [
  "web/shared/contracts.ts", `${owned}/run.mjs`, `${owned}/fixture.mjs`,
  `${owned}/fixture.html`, `${owned}/monitor.js`,
]) implementationFiles.push({ path, sha256: digest(await readFile(path)) });

const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, "http://127.0.0.1");
    let data, type = "application/octet-stream";
    if (url.pathname.startsWith("/asset/")) {
      const id = decodeURIComponent(url.pathname.slice(7));
      const path = registry.get(id);
      if (!path) { response.writeHead(404).end("Unknown source input"); return; }
      requests.push({ id, at: Date.now() });
      const fault = faults.get(id);
      if (fault) faults.delete(id);
      if (fault?.kind === "http") { response.writeHead(503).end("Injected loopback network failure"); return; }
      if (fault?.kind === "delay") await new Promise((resolve) => setTimeout(resolve, fault.ms));
      data = await readFile(path);
      if (fault?.kind === "bytes") {
        data = Buffer.from(data);
        data[0] ^= 1;
      }
      if (fault?.kind === "truncated") data = data.subarray(0, data.length - 1);
      type = path.endsWith(".json") ? "application/json" : path.endsWith(".wav") ? "audio/wav" : "audio/flac";
    } else if (modules.has(url.pathname)) {
      data = modules.get(url.pathname);
      type = "text/javascript";
    } else if (staticFiles.has(url.pathname)) {
      const path = staticFiles.get(url.pathname);
      data = await readFile(`${owned}/${path}`);
      type = path.endsWith(".html") ? "text/html" : "text/javascript";
    } else if (url.pathname === "/health") {
      data = "loopback audio fixture ready"; type = "text/plain";
    } else if (url.pathname === "/favicon.ico") {
      response.writeHead(204).end(); return;
    } else {
      response.writeHead(404).end("No such fixture route"); return;
    }
    response.writeHead(200, {
      "content-type": type, "content-length": Buffer.byteLength(data),
      "cache-control": "no-store", "x-content-type-options": "nosniff",
    });
    response.end(data);
  } catch (error) {
    if (!response.headersSent) response.writeHead(500);
    response.end(String(error));
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
assert.equal((await fetch(`${origin}/health`)).status, 200, "The owned server must actually be responsive.");
console.log(`Responsive loopback fixture: ${origin}/health`);
await mkdir(run, { recursive: true });
for (const path of ["profile", "home", "cache", "config"]) await mkdir(`${run}/${path}`, { recursive: true });
// A short existing OWNED directory keeps Chromium's Unix socket below 108
// bytes. No /tmp, global browser profile, audio service, driver or sandbox edit.
const scratchBefore = new Set(await readdir("web/audio"));
process.env.TMPDIR = resolve("web/audio");
let context;
let page;
const results = [];
const report = {
  schemaVersion: 1, fixtureOnly: true, sourcePackSha256: "b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d",
  browser: version, node: process.version, startedAt: new Date().toISOString(),
  sourceAssets: sources.length, sourceFilesChanged: null, sandbox: null, audioDevice: null,
  implementationFiles,
  headless: true, hostSpeakerPerception: "not tested", macEdge: "not tested",
  validationScope: process.argv.includes("--quick") ? "lifecycle-smoke-with-offline-boundary" : "full-browser-audio-control-and-native-playlist-boundary",
  gameplayOrM1Acceptance: false, results, failures: [], complete: false,
};

function wavSamples(bytes) {
  assert.equal(bytes.toString("ascii", 0, 4), "RIFF");
  assert.equal(bytes.toString("ascii", 8, 12), "WAVE");
  let pcm;
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const length = bytes.readUInt32LE(offset + 4);
    if (bytes.toString("ascii", offset, offset + 4) === "fmt ") {
      assert.equal(bytes.readUInt16LE(offset + 8), 1);
      assert.equal(bytes.readUInt16LE(offset + 10), 1);
      assert.equal(bytes.readUInt32LE(offset + 12), 22050);
      assert.equal(bytes.readUInt16LE(offset + 22), 16);
    }
    if (bytes.toString("ascii", offset, offset + 4) === "data") pcm = bytes.subarray(offset + 8, offset + 8 + length);
    offset += 8 + length + (length & 1);
  }
  assert.ok(pcm);
  return Array.from({ length: pcm.length / 2 }, (_, i) => pcm.readInt16LE(i * 2) / 32768);
}
const originalRat = wavSamples(await readFile(registry.get("reference.audio.sfx.710")));
const nativePreferences = JSON.parse(await readFile("research/browser-audio-policy/native-preferences.json", "utf8"));
const nativePosition = JSON.parse(await readFile("research/browser-audio-policy/native-position.json", "utf8"));
const nativeMusic = JSON.parse(await readFile("research/browser-audio-policy/native-music.json", "utf8"));
const nativeCurve = nativePreferences.cases.find((item) => item.case === "native-nonlinear-lookup-tables");

async function check(name, body) {
  const start = Date.now();
  const detail = await body();
  results.push({ name, passed: true, durationMs: Date.now() - start, ...detail });
  console.log(`PASS ${name}`);
}
async function snapshot() { return page.evaluate(() => audioFixture.snapshot()); }
async function waitVoice(sourceId, kind = null) {
  await page.waitForFunction(({ id, kind }) => audioFixture.snapshot().voices.some(
    (v) => v.sourceId === id && v.when <= audioFixture.context.currentTime && (kind === null || v.kind === kind),
  ), { id: sourceId, kind }, { timeout: 20_000 });
}
async function waitEnded(eventId) {
  await page.waitForFunction((id) => audioFixture.snapshot().traces.some(
    (t) => t.type === "ended" && t.data.eventId === id && t.data.natural === true,
  ), eventId, { timeout: 12_000 });
}
async function emit(overrides) {
  return page.evaluate((overrides) => {
    const event = audioFixture.event(overrides);
    audioFixture.update(audioFixture.world, [event]);
    return event.id;
  }, overrides);
}
async function gain(channel, value) {
  await page.locator(`#${channel}`).focus();
  await page.keyboard.press("Home");
  for (let i = 0; i < value; i++) await page.keyboard.press("ArrowRight");
  assert.equal((await snapshot()).volumes[channel], value / 100);
}
async function trustedUnlock() {
  await page.click("#unlock");
  const result = await page.evaluate(() => audioFixture.lastUnlock);
  assert.equal(result.success, true, JSON.stringify(result));
  assert.equal(result.state, "running");
}

try {
  context = await chromium.launchPersistentContext(resolve(`${run}/profile`), {
    executablePath: chrome, headless: true, chromiumSandbox: true,
    ignoreDefaultArgs: ["--mute-audio"],
    args: ["--enable-automation", "--autoplay-policy=document-user-activation-required"],
    viewport: { width: 1280, height: 800 },
    serviceWorkers: "block",
    env: {
      ...process.env, HOME: resolve(`${run}/home`), TMPDIR: resolve("web/audio"),
      XDG_CACHE_HOME: resolve(`${run}/cache`), XDG_CONFIG_HOME: resolve(`${run}/config`),
    },
  });
  await context.route("**/*", (route) => {
    const url = route.request().url();
    return url.startsWith(`${origin}/`) ? route.continue() : route.abort("blockedbyclient");
  });
  page = await context.newPage();
  page.setDefaultTimeout(20_000);
  page.on("pageerror", (error) => report.failures.push(`pageerror: ${error.message}`));
  const cdp = await context.newCDPSession(page);
  // Playwright's ordinary evaluate uses CDP userGesture:true. That would make
  // an autoplay test a false positive. All fixture orchestration/observation
  // below uses gesture-free CDP; only mouse/keyboard input may unlock audio.
  page.evaluate = async (fn, arg) => {
    const result = await cdp.send("Runtime.evaluate", {
      expression: `(${fn.toString()})(${JSON.stringify(arg) ?? ""})`,
      userGesture: false, awaitPromise: true, returnByValue: true,
    });
    if (result.exceptionDetails) throw new Error(
      result.exceptionDetails.exception?.description ?? result.exceptionDetails.text,
    );
    return result.result.value;
  };
  page.waitForFunction = async (fn, arg, options = {}) => {
    const end = Date.now() + (options.timeout ?? 20_000);
    while (Date.now() < end) {
      if (await page.evaluate(fn, arg)) return;
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
    throw new Error(`Gesture-free browser wait timed out: ${fn.toString()}`);
  };
  const commandLine = (await cdp.send("Browser.getBrowserCommandLine")).arguments;
  assert.equal(commandLine.some((arg) => arg === "--no-sandbox" || arg === "--mute-audio"), false);
  const sandboxPage = await context.newPage();
  await sandboxPage.goto("chrome://sandbox");
  const sandbox = await sandboxPage.locator("body").innerText();
  assert.match(sandbox, /Layer 1 Sandbox\s+Namespace/);
  assert.match(sandbox, /PID namespaces\s+Yes/);
  assert.match(sandbox, /Network namespaces\s+Yes/);
  assert.match(sandbox, /Seccomp-BPF sandbox\s+Yes/);
  report.sandbox = { namespace: true, seccompBpf: true, noSandboxFlag: false, muteAudioFlag: false };
  await sandboxPage.close();
  await page.goto(origin);
  await page.evaluate(() => audioFixture.ready);

  await check("explicit permission: no startup audio request or source before a real gesture", async () => {
    const before = await snapshot();
    assert.equal(before.pendingGesture, true);
    assert.equal(before.unlocked, false);
    assert.equal(before.voices.length, 0);
    assert.equal(requests.filter((r) => r.id.startsWith("asset.") || r.id.startsWith("reference.")).length, 0);
    const synthetic = await page.evaluate(async () => {
      try { await audioFixture.handle.unlock(); return "incorrect-success"; }
      catch (error) { return error.code; }
    });
    assert.equal(synthetic, "AUDIO_GESTURE_REQUIRED");
    const syntheticClick = await page.evaluate(async () => {
      document.querySelector("#unlock").click();
      return audioFixture.lastUnlock;
    });
    assert.equal(syntheticClick.success, false);
    assert.equal(syntheticClick.code, "AUDIO_GESTURE_REQUIRED");
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), 0);
    return { actualContextState: before.contextState, syntheticGestureRejected: true, syntheticClickRejected: true, scriptUserGesture: false, requestedPlayableFiles: 0 };
  });

  await check("mouse unlock starts original Scape Main 0 through the enabled destination", async () => {
    await trustedUnlock();
    await waitVoice(0, "music");
    const a = await snapshot();
    await page.waitForTimeout(750);
    const b = await snapshot();
    assert.ok(b.currentTime > a.currentTime + 0.5);
    assert.equal(b.pendingGesture, false);
    assert.equal(b.voices.filter((v) => v.kind === "music").length, 1);
    assert.equal(b.voices[0].loop, false);
    assert.equal(await page.evaluate(() => audioFixture.originalsConnected()), true);
    const monitor = await page.evaluate(() => audioFixture.stats());
    assert.ok(monitor.nonzero > 0);
    assert.equal(monitor.clips, 0);
    const playback = await page.evaluate(() => ({
      nativeStarts: audioFixture.native.starts, sampleRate: audioFixture.context.sampleRate,
      state: audioFixture.context.state, baseLatency: audioFixture.context.baseLatency,
      outputLatency: audioFixture.context.outputLatency, sinkId: audioFixture.context.sinkId ?? null,
    }));
    assert.equal(playback.nativeStarts.length, 1);
    assert.equal(playback.nativeStarts[0].state, "running");
    assert.equal(playback.nativeStarts[0].frames, manifest.assets.find((a) => a.kind === "music" && a.source_group === 0).signal.frames);
    report.audioDevice = { ...playback, nativeStarts: undefined };
    return { nativeStart: playback.nativeStarts[0], monitor, actualTimeAdvance: b.currentTime - a.currentTime };
  });

  await check("actual native defaults, master-before-lookup slider composition and qualified Modern geography", async () => {
    assert.deepEqual((await snapshot()).nativeMixer, { music: 255, effects: 127, area: 127 });
    assert.deepEqual(await page.evaluate(() => audioFixture.sourceAudioDefaults().assetGain),
      { music: 255/128, effects: 127/128, area: 127/128 });
    await page.locator("#master").focus();
    await page.keyboard.press("Home");
    for (let i=0;i<50;i++) await page.keyboard.press("ArrowRight");
    assert.deepEqual((await snapshot()).nativeMixer, { music: 44, effects: 22, area: 22 });
    await page.keyboard.press("End");
    await waitVoice(0);
    const regions = await page.evaluate(() => [
      audioFixture.sourceMusicRegion({ x:3222,y:3218,plane:0 }),
      audioFixture.sourceMusicRegion({ x:3222,y:3280,plane:0 }),
      audioFixture.sourceMusicRegion({ x:3166,y:3300,plane:0 }),
    ]);
    assert.ok(regions.every((r) => r.areaId===1 && r.defaultGroup===76));
    assert.ok(regions.every((r) => JSON.stringify(r.groups)==="[2,64,327,163,76,145]"));
    return { nativeDefaultMixer: { music:255,effects:127,area:127 }, nativeMaster50Mixer: { music:44,effects:22,area:22 },
      modernArea:1, originalGroups:regions[0].groups, publicPolygonRevision:15258397,
      classification:"dated_public_source_geography_with_native_music_table_corroboration" };
  });

  await check("needed-region music 62 / 144 / 76 / east 2, repeated snapshots, reset and reconnect", async () => {
    const routes = [
      ["region.osrs.12336", 62], ["region.osrs.12436", 144],
      ["region.osrs.12850", 76], ["region.osrs.12851", 2],
    ];
    const proof = [];
    for (const [region, group] of routes) {
      await page.evaluate((region) => {
        const world = audioFixture.syntheticWorld(region);
        world.revision = String(BigInt(audioFixture.world?.revision ?? "0") + 1n);
        audioFixture.update(world);
      }, region);
      await waitVoice(group, "music");
      const starts = await page.evaluate(() => audioFixture.native.starts.length);
      await page.evaluate(() => {
        for (let i = 0; i < 5; i++) audioFixture.update(audioFixture.world);
      });
      await page.waitForTimeout(100);
      const state = await snapshot();
      assert.equal(state.voices.filter((v) => v.kind === "music").length, 1);
      assert.equal(await page.evaluate(() => audioFixture.native.starts.length), starts);
      proof.push({ region, group, sourceStarted: true, duplicateSnapshotStarts: 0 });
    }
    await page.evaluate(() => audioFixture.handle.disconnected());
    assert.equal((await snapshot()).voices.length, 0);
    const disconnectedStarts = await page.evaluate(() => audioFixture.native.starts.length);
    await page.waitForTimeout(150);
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), disconnectedStarts);
    await page.evaluate(() => audioFixture.update(audioFixture.world));
    await waitVoice(2, "music");
    assert.equal((await snapshot()).voices.filter((v) => v.kind === "music").length, 1);
    return { routes: proof, reconnectBackgroundSources: 1, pausedNetworkSources: 0 };
  });

  await check("same-track Tutorial squares do not restart music, and a region change preserves the active jingle", async () => {
    await page.evaluate(() => {
      const world = audioFixture.syntheticWorld("region.osrs.12336");
      world.revision = "20";
      audioFixture.update(world);
    });
    await waitVoice(62, "music");
    const starts = await page.evaluate(() => audioFixture.native.starts.length);
    await page.evaluate(() => {
      const world = audioFixture.syntheticWorld("region.osrs.12592");
      world.revision = "21";
      audioFixture.update(world);
    });
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), starts);
    const jingleId = await emit({ kind: "jingle", sourceId: 33, payload: { committed: true } });
    await waitVoice(33, "jingle");
    await page.evaluate(() => {
      const world = audioFixture.syntheticWorld("region.osrs.12851");
      world.revision = "22";
      audioFixture.update(world);
    });
    const state = await snapshot();
    assert.equal(state.voices.length, 1);
    assert.equal(state.voices[0].eventId, jingleId);
    await waitEnded(jingleId);
    await waitVoice(2, "music");
    return { sameGroupRestarts: 0, jingleInterruptedByRegion: false, newRememberedBackground: 2 };
  });

  await gain("music", 0);
  await gain("area", 0);

  await check("real original shortbow, rat, goblin and smelt cues start and naturally end", async () => {
    const bindings = [
      ["shortbow_release", 2693], ["rat_attack", 710], ["rat_hit", 713], ["rat_death", 711],
      ["goblin_attack", 469], ["goblin_hit", 472], ["goblin_death", 471], ["bronze_smelt_start", 2725],
    ];
    const proof = [];
    for (const [selector, sourceId] of bindings) {
      const id = await emit({
        sourceId, payload: { committed: true, selector, repeatCount: 1, delayCycles: 2 },
      });
      await waitEnded(id);
      const state = await snapshot();
      const start = state.traces.find((t) => t.type === "started" && t.data.eventId === id);
      const end = state.traces.find((t) => t.type === "ended" && t.data.eventId === id);
      assert.ok(start && end);
      assert.equal(start.data.sourceId, sourceId);
      assert.equal(start.data.loop, false);
      proof.push({ selector, sourceId, start: start.data.when, end: end.audioTime, nativeNaturalEnd: true });
    }
    return { bindings: proof, monitor: await page.evaluate(() => audioFixture.stats()) };
  });

  await check("live monitor waveform and real slider gain match the original rat WAV within approved float tolerance", async () => {
    const measurements = [];
    for (const percent of [100, 50]) {
      await gain("effects", percent);
      const captured = await page.evaluate(async () => {
        return audioFixture.capture(22050, () => {
          const e = audioFixture.event({
            sourceId: 710, payload: { committed: true, selector: "rat_attack", repeatCount: 1, delayCycles: 2 },
          });
          audioFixture.update(audioFixture.world, [e]);
        });
      });
      const nativeVolume = nativeCurve.effect_and_area[Math.round(percent / 100 * 127)];
      const expected = originalRat.map((v) => v * nativeVolume / 256);
      const first = expected.findIndex((v) => v !== 0);
      const actualFirst = captured.samples.findIndex((v) => Math.abs(v) > 1e-8);
      const shift = actualFirst - first;
      assert.ok(shift >= 0 && shift <= 2205, `Unexpected actual waveform delay ${shift} samples`);
      let maxError = 0, peak = 0, clips = 0;
      for (let i = 0; i < expected.length; i++) {
        const actual = captured.samples[i + shift];
        assert.notEqual(actual, undefined);
        maxError = Math.max(maxError, Math.abs(actual - expected[i]));
        peak = Math.max(peak, Math.abs(actual));
        if (Math.abs(actual) > 1) clips++;
      }
      assert.ok(maxError <= 0.000023, `Original PCM/gain mismatch: ${maxError}`);
      assert.equal(clips, 0);
      measurements.push({ sliderPercent: percent, nativeVolume, assetGain: nativeVolume / 128,
        maxAbsoluteError: maxError, peak, extraClipSamples: clips, shiftSamples: shift });
    }
    assert.ok(Math.abs(20 * Math.log10(measurements[1].peak / measurements[0].peak) -
      20 * Math.log10(measurements[1].nativeVolume / measurements[0].nativeVolume)) < 0.25);
    return { measurements, originalWavSha256: reference.reference_templates.find((a) => a.source_group === 710).sha256 };
  });
  await gain("effects", 100);

  await check("actual source animation cues are activity-prepared, scheduled on source cycles, and cancel cleanly", async () => {
    await gain("area", 100);
    const expected = new Map([
      [621, [[4, 2603, 16]]], [625, [[11, 3220, 50]]],
      [733, [[8, 2597, 45], [10, 2597, 70]]], [879, [[3, 2735, 18]]],
      [898, [[5, 3790, 20], [7, 3791, 41], [9, 3790, 62], [11, 3791, 83]]],
    ]);
    const proof = [];
    for (const [sequence, cues] of expected) {
      const action = `gather/${sequence}`;
      const prepared = await emit({
        kind: "animation", sourceId: sequence,
        payload: { committed: true, phase: "prepare", actionId: action },
      });
      const needed = [...new Set(cues.map((cue) => cue[1]))];
      await page.waitForFunction(({ id, needed }) => needed.every((sourceId) => audioFixture.snapshot().traces.some(
        (t) => t.type === "prepared" && t.data.eventId === id && t.data.sourceId === sourceId,
      )), { id: prepared, needed });
      const start = await page.evaluate(({ sequence, action }) => {
        const now = audioFixture.context.currentTime;
        const event = audioFixture.event({
          kind: "animation", sourceId: sequence,
          payload: { committed: true, phase: "start", actionId: action },
        });
        audioFixture.update(audioFixture.world, [event]);
        return { now, id: event.id };
      }, { sequence, action });
      await page.waitForFunction(({ id, count }) => audioFixture.snapshot().traces.filter(
        (t) => t.type === "effect_dispatched" && t.data.eventId === id,
      ).length === count, { id: start.id, count: cues.length });
      const dispatches = (await snapshot()).traces.filter((t) => t.type === "effect_dispatched" && t.data.eventId === start.id);
      for (let i = 0; i < cues.length; i++) {
        assert.equal(dispatches[i].data.sourceId, cues[i][1]);
        assert.equal(dispatches[i].data.processingCalls, cues[i][2]);
        const errorMs = Math.abs((dispatches[i].data.when - start.now) * 1000 - cues[i][2] * 20);
        assert.ok(errorMs <= 20, `Source sequence ${sequence}, frame ${cues[i][0]}: ${errorMs}ms`);
        proof.push({ sequence, frame: cues[i][0], sound: cues[i][1], sourceCycles: cues[i][2], processingCalls: dispatches[i].data.processingCalls, errorMs });
      }
      await emit({
        kind: "animation", sourceId: sequence,
        payload: { committed: true, phase: "cancel", actionId: action },
      });
    }
    const starts = await page.evaluate(() => audioFixture.native.starts.length);
    await emit({
      kind: "animation", sourceId: 898,
      payload: { committed: true, phase: "start", actionId: "cancel-before-frame" },
    });
    await emit({
      kind: "animation", sourceId: 898,
      payload: { committed: true, phase: "cancel", actionId: "cancel-before-frame" },
    });
    await page.waitForTimeout(450);
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), starts);
    return { cues: proof, cancelledActionStartedSources: 0 };
  });

  await check("native countdown timing and approved eating frame-1 offset, stable callback deduplication", async () => {
    await gain("area", 100);
    const warm = await emit({
      kind: "animation", sourceId: 12526,
      payload: { committed: true, itemId: 315, phase: "start", actionId: "warm-food" },
    });
    await waitEnded(warm);
    const result = await page.evaluate(async () => {
      const now = audioFixture.context.currentTime;
      const action = "once-eating-2393";
      const e = audioFixture.event({
        id: "food/committed", kind: "animation", sourceId: 12526, sourceCycle: 100,
        payload: { committed: true, itemId: 2309, phase: "start", actionId: action },
      });
      const duplicate = audioFixture.event({
        id: "food/frame-callback", kind: "animation", sourceId: 12526, sourceCycle: 104,
        payload: { committed: true, itemId: 2309, phase: "frame", frame: 1, actionId: action },
      });
      const capture = audioFixture.capture(33075, () => {
        audioFixture.update(audioFixture.world, [e, e, duplicate]);
      });
      const pcm = await capture;
      return { now, capture: pcm, state: audioFixture.snapshot() };
    });
    const starts = result.state.traces.filter((t) => t.type === "started" && t.data.eventId === "food/committed");
    assert.equal(starts.length, 1);
    const dispatch = result.state.traces.find((t) => t.type === "effect_dispatched" && t.data.eventId === "food/committed");
    assert.ok(dispatch.data.when - result.now >= 0.06 - 0.001);
    assert.ok(dispatch.data.when - result.now <= 0.10 + 0.001);
    assert.ok(dispatch.data.lateMs <= 20);
    assert.equal(dispatch.data.processingCalls, 4);
    const first = result.capture.samples.findIndex((value) => value !== 0);
    const onset = (result.capture.startFrame + first) / 22050;
    const expected = dispatch.data.when + 9393 / 22050;
    assert.ok(Math.abs(onset - expected) <= 0.020, `Baked eating offset was added twice: ${onset} vs ${expected}`);
    await page.evaluate(() => {
      audioFixture.handle.disconnected();
      audioFixture.update(audioFixture.world, [{
        id: "food/committed", kind: "animation", sourceId: 12526, assetId: null,
        actorId: audioFixture.world.player.id, tile: null, sourceCycle: 100,
        payload: { committed: true, itemId: 2309, phase: "start", actionId: "once-eating-2393" },
      }]);
    });
    await page.waitForTimeout(150);
    assert.equal((await snapshot()).voices.filter((v) => v.sourceId === 2393).length, 0);
    return {
      sourceStartDelayMs: (dispatch.data.when - result.now) * 1000, sourceCycleLateMs: dispatch.data.lateMs,
      actualWaveformOnset: onset, expectedWithExactlyOneBakedOffset: expected,
      onsetErrorMs: Math.abs(onset - expected) * 1000, duplicateSources: 0,
      processingCalls: dispatch.data.processingCalls,
    };
  });

  await check("source-silence 2411 does not request a missing asset or renormalize to an audible alternative", async () => {
    const before = requests.length;
    const e = await emit({
      kind: "animation", sourceId: 13612,
      payload: { committed: true, phase: "start", weightRoll: 73, actionId: "silent-branch" },
    });
    await page.waitForTimeout(200);
    assert.equal(requests.length, before);
    assert.equal((await snapshot()).traces.filter((t) => t.type === "source_silence" && t.data.eventId === e).length, 1);
    await page.evaluate(() => {
      audioFixture.update(audioFixture.world, Array.from({ length: 51 }, (_, i) => audioFixture.event({
        id: `silent-fifo/${i}`, sourceId: 2411,
        payload: { committed: true, delayCycles: 2, repeatCount: 1 },
      })));
    });
    assert.equal((await snapshot()).queueSize, 50);
    await page.waitForTimeout(120);
    assert.equal((await snapshot()).traces.filter((t) => t.type === "source_silence" && String(t.data.eventId).startsWith("silent-fifo/")).length, 50);
    assert.equal(requests.length, before);
    return { originalSilentWeight: 74, testedRoll: 73, playableRequests: 0, silenceStillOccupiesNativeFifo: true };
  });

  await check("50-entry FIFO overflow is preserved on real sources with no stress-fixture clipping", async () => {
    const clipBaseline = (await page.evaluate(() => audioFixture.stats())).clips;
    const before = await page.evaluate(() => audioFixture.native.starts.length);
    await page.evaluate(() => {
      const events = Array.from({ length: 51 }, (_, i) => audioFixture.event({
        id: `fifo/${i}`, sourceId: 710,
        payload: { committed: true, selector: "rat_attack", repeatCount: 1, delayCycles: 2, sourceGain: 0.01 },
      }));
      audioFixture.update(audioFixture.world, events);
    });
    assert.equal((await snapshot()).queueSize, 50);
    await waitEnded("fifo/49");
    const state = await snapshot();
    const started = state.traces.filter((t) => t.type === "started" && String(t.data.eventId).startsWith("fifo/"));
    assert.deepEqual(started.map((t) => t.data.eventId), Array.from({ length: 50 }, (_, i) => `fifo/${i}`));
    assert.equal(state.traces.filter((t) => t.type === "effect_dispatched" && String(t.data.eventId).startsWith("fifo/"))
      .every((t) => t.data.processingCalls === 3), true);
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), before + 50);
    const monitor = await page.evaluate(() => audioFixture.stats());
    assert.equal(monitor.clips - clipBaseline, 0);
    return { capacity: 50, submitted: 51, started: 50, newEntryDropped: "fifo/50", processingCallsForDelay2: 3, perEventFixtureGain: 0.01, additionalClipSamples: monitor.clips - clipBaseline };
  });

  await check("native scene emitter gain, footprint, signed fades, plane/instance and movement without sourceGain", async () => {
    await gain("music", 0);
    await gain("area", 100);
    const tile = await page.evaluate(() => ({ ...audioFixture.world.player.tile }));
    const scene = {
      listener: { x: tile.x * 128 + 64, y: tile.y * 128 + 64 },
      plane: tile.plane, instance: null, owner: null,
      emitters: [{ id: "native-range", objectId: 114, tile, orientation: 0, instance: null, owner: null, present: true }],
    };
    const sendScene = (value) => page.evaluate((value) => {
      audioFixture.setSourceAudioScene({ ...value, varps: new Map() });
    }, value);
    await sendScene(scene);
    await waitVoice(2065, "sfx");
    const state = await snapshot();
    const voice = state.voices.find((v) => v.sourceId === 2065);
    assert.equal(voice.loop, true);
    assert.equal(voice.loopEnd, 2);
    assert.equal(voice.gain, 127 / 128);
    assert.equal(state.queueSize, 0, "Native object streams do not occupy the packet FIFO");
    const starts = await page.evaluate(() => audioFixture.native.starts.length);
    await sendScene(scene);
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), starts);
    const distant = { ...scene, listener: { x: (tile.x + 1) * 128 + 192, y: tile.y * 128 + 64 } };
    await sendScene(distant);
    await page.waitForTimeout(180);
    assert.equal((await snapshot()).voices.find((v) => v.sourceId === 2065).id, voice.id);
    assert.equal((await snapshot()).voices.find((v) => v.sourceId === 2065).gain, 85 / 128);
    await sendScene(scene);
    await page.waitForTimeout(40);
    assert.equal((await snapshot()).voices.find((v) => v.sourceId === 2065).gain, 127 / 128);
    await sendScene({ ...scene, plane: 1 });
    await page.waitForTimeout(200);
    assert.equal((await snapshot()).voices.some((v) => v.sourceId === 2065), false);
    const beforeInvalid = await page.evaluate(() => audioFixture.native.starts.length);
    await sendScene({ ...scene, emitters: [{ ...scene.emitters[0], instance: "other-instance" }] });
    await page.waitForTimeout(60);
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), beforeInvalid);
    await page.evaluate(() => audioFixture.setSourceAudioScene(null));
    return {
      originalObject: 114, originalFootprint: [1, 2], originalLoopEndSeconds: 2,
      nativeMixerVolumes: [127, 85, 127], unchangedVoiceId: voice.id,
      callerSuppliedSourceGain: false, normalFadeBaseMs: 300, visibilityFadeMs: 150,
      wrongPlaneOrInstanceSources: 0, packetFifoOccupancy: 0,
    };
  });

  await check("native morph variables suppress inactive base sounds and random ambience runs independently of the FIFO", async () => {
    const tile = await page.evaluate(() => ({ ...audioFixture.world.player.tile }));
    const scene = {
      listener: { x:tile.x*128+64,y:tile.y*128+64 }, plane:tile.plane, instance:null, owner:null,
      emitters: [{ id:"native-morph",objectId:34815,tile,orientation:0,instance:null,owner:null,present:true }],
    };
    const before = requests.length;
    await page.evaluate((scene) => audioFixture.setSourceAudioScene({ ...scene,varps:new Map([[491,0]]) }), scene);
    await page.waitForTimeout(80);
    assert.equal((await snapshot()).voices.some((v) => v.sourceId===3141), false);
    await page.evaluate((scene) => audioFixture.setSourceAudioScene({ ...scene,varps:new Map([[491,4]]) }), scene);
    await page.waitForTimeout(80);
    assert.equal(requests.length,before);
    const randomScene = { ...scene,emitters:[{ ...scene.emitters[0],id:"native-random",objectId:16433 }] };
    await page.evaluate((scene) => audioFixture.setSourceAudioScene({ ...scene,varps:new Map() }), randomScene);
    await waitVoice(2184);
    await page.waitForFunction(() => audioFixture.snapshot().voices.some(
      (v) => [1984,1985,1986,1987,1988].includes(v.sourceId)), null, { timeout:10_000 });
    assert.equal((await snapshot()).queueSize,0);
    await page.evaluate(() => audioFixture.setSourceAudioScene(null));
    assert.equal((await snapshot()).voices.some((v) => v.kind==="sfx"),false);
    return { originalMorph:34815,sourceVarp:491,values:[0,4],inactiveBaseSoundNotPlayed:3141,
      randomObject:16433,originalRandomGroups:[1984,1985,1986,1987,1988],sourceIntervalRangeCycles:[150,300],
      packetFifoOccupancy:0 };
  });

  await check("last accepted jingle wins in both directions; sentinel and auxiliary values have no priority", async () => {
    await gain("music", 100);
    const proof = [];
    for (const groups of [[154, 33, -1], [33, 154, -1]]) {
      const before = await page.evaluate(() => audioFixture.native.starts.length);
      await page.evaluate((groups) => {
        audioFixture.update(audioFixture.world, groups.map((sourceId, index) => audioFixture.event({
          kind: "jingle", sourceId, payload: { committed: true, auxiliary: index === 0 ? 60000 : 0 },
        })));
      }, groups);
      const winner = groups[1];
      await waitVoice(winner, "jingle");
      const state = await snapshot();
      assert.equal(state.voices.filter((v) => v.channel === "music").length, 1);
      assert.equal(await page.evaluate(() => audioFixture.native.starts.length), before + 1);
      proof.push({ requests: groups, playing: winner, simultaneousMusicalSources: 1 });
    }
    await emit({ kind: "jingle", sourceId: 33, payload: { committed: true, auxiliary: 255 } });
    await waitVoice(33, "jingle");
    await page.waitForFunction(() => audioFixture.snapshot().voices.some((v) => v.kind === "music" && v.sourceId === 2), null, { timeout: 10_000 });
    return { cases: proof, resumedRememberedBackground: 2, resumedFromFrame: 0 };
  });

  await check("legitimate once-only Learning/Cook completion and modal-deferred Cook reward jingle", async () => {
    const complete = (quest, id) => page.evaluate(({ quest, id }) => {
      const world = audioFixture.world;
      world.revision = String(BigInt(world.revision) + 1n);
      world.player.quests.find((q) => q.id === quest).completed = true;
      audioFixture.update(world, [audioFixture.event({
        id, kind: "quest_complete", sourceId: null, payload: { committed: true, questId: quest },
      })]);
    }, { quest, id });
    await complete("quest.learning_the_ropes", "learning/committed");
    await waitVoice(152, "jingle");
    const before = await page.evaluate(() => audioFixture.native.starts.length);
    await complete("quest.learning_the_ropes", "learning/duplicate-new-id");
    await page.waitForTimeout(100);
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), before);
    await complete("quest.cooks_assistant", "cook/committed");
    await waitVoice(152, "jingle");
    const rewardId = await emit({
      kind: "level_up", sourceId: 33, payload: { committed: true, level: 4, causeQuestId: "quest.cooks_assistant" },
    });
    await page.waitForTimeout(100);
    assert.equal((await snapshot()).voices.some((v) => v.sourceId === 33), false);
    await emit({
      kind: "interface_closed", sourceId: 153,
      payload: { questId: "quest.cooks_assistant", completionId: "cook/committed" },
    });
    await waitVoice(33, "jingle");
    const state = await snapshot();
    assert.equal(state.traces.filter((t) => t.type === "started" && t.data.eventId === rewardId).length, 1);
    await page.evaluate(() => {
      audioFixture.handle.disconnected();
      audioFixture.update(audioFixture.world, [audioFixture.event({
        kind: "quest_complete", sourceId: null,
        payload: { committed: true, questId: "quest.learning_the_ropes" },
      })]);
    });
    await waitVoice(2, "music");
    assert.equal((await snapshot()).voices.some((v) => v.kind === "jingle"), false);
    return { learningClassification: "approved_adaptation", learningReplaySources: 0, cookJingle: 152, rewardAfterSourceScroll153: 33, reconnectReplaySources: 0 };
  });

  await check("mute/unmute and zero-volume admission never leak nodes or replay rejected effects", async () => {
    await page.click("#mute");
    assert.equal((await snapshot()).voices.length, 0);
    await emit({ sourceId: 710 });
    await emit({ kind: "jingle", sourceId: 154, payload: { committed: true } });
    assert.equal((await snapshot()).queueSize, 0);
    const silence = await page.evaluate(() => audioFixture.capture(4410));
    assert.equal(silence.samples.some((value) => value !== 0), false);
    await page.click("#unmute");
    await waitVoice(2, "music");
    const state = await snapshot();
    assert.equal(state.voices.length, 1);
    assert.equal(state.voices[0].kind, "music");
    await gain("effects", 0);
    await emit({ sourceId: 710 });
    assert.equal((await snapshot()).queueSize, 0);
    await gain("effects", 100);
    assert.equal((await snapshot()).voices.filter((v) => v.kind === "sfx").length, 0);
    return { mutedSamplesNonzero: 0, musicAfterUnmute: 1, rejectedEffectReplay: 0 };
  });

  await check("untrusted IDs, selectors, source bounds and noncommitted actions fail explicitly without fetching audio", async () => {
    const before = requests.length;
    const starts = await page.evaluate(() => audioFixture.native.starts.length);
    await emit({ sourceId: 65000 });
    await emit({ sourceId: 710, assetId: "asset.source.osrs.cache2695.audio-runtime.sfx.2393" });
    await emit({ sourceId: 710, payload: { committed: false, delayCycles: 0, repeatCount: 1 } });
    await emit({ sourceId: 2266 });
    await emit({ sourceId: 2725, payload: { committed: true, selector: "shortbow_release", delayCycles: 0, repeatCount: 1 } });
    await emit({ kind: "animation", sourceId: 426, payload: { committed: true, phase: "start" } });
    await emit({ sourceId: 710, payload: { committed: true, delayCycles: -1, repeatCount: 1 } });
    await page.evaluate(() => audioFixture.handle.volume("effects", NaN));
    assert.equal(requests.length, before);
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), starts);
    const codes = await page.evaluate(() => [...new Set(audioFixture.errors.map((e) => e.code))]);
    for (const code of ["AUDIO_ASSET_ID", "AUDIO_EVENT", "AUDIO_BINDING", "AUDIO_VOLUME"]) assert.ok(codes.includes(code));
    return { rejectedCases: 8, additionalPlayableFetches: 0, additionalSourceStarts: 0 };
  });

  await check("real suspend freezes context time; injected resume rejection reports failure; keyboard gesture recovers the same music node", async () => {
    const before = await page.evaluate(() => ({
      starts: audioFixture.native.starts.length, running: audioFixture.context.state,
    }));
    await page.evaluate(() => audioFixture.context.suspend());
    await page.waitForFunction(() => audioFixture.snapshot().pendingGesture);
    const a = (await snapshot()).currentTime;
    await page.waitForTimeout(120);
    assert.equal((await snapshot()).currentTime, a);
    await page.evaluate(() => {
      const context = audioFixture.context;
      context.savedResume = context.resume;
      context.resume = () => Promise.reject(new DOMException("Injected device resume rejection", "NotAllowedError"));
    });
    await page.click("#unlock");
    const rejected = await page.evaluate(() => audioFixture.lastUnlock);
    assert.equal(rejected.success, false);
    assert.equal((await snapshot()).pendingGesture, true);
    await page.evaluate(() => {
      const context = audioFixture.context;
      context.resume = context.savedResume;
      delete context.savedResume;
    });
    await page.locator("#unlock").focus();
    await page.keyboard.press("Enter");
    assert.equal((await page.evaluate(() => audioFixture.lastUnlock)).success, true);
    await page.waitForTimeout(150);
    assert.ok((await snapshot()).currentTime > a + 0.1);
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), before.starts);
    return { realSuspension: true, frozenTime: a, resumeFaultInjected: true, keyboardRecovered: true, duplicateMusicSources: 0 };
  });

  await check("malformed source bytes, HTTP failure and truncation are not decode/playback success", async () => {
    const unused = manifest.assets.filter((a) => a.kind === "sfx" &&
      !requests.some((request) => request.id === a.asset_id)).slice(0, 3);
    const proof = [];
    for (const [index, kind] of ["bytes", "http", "truncated"].entries()) {
      const asset = unused[index];
      const before = await page.evaluate(() => audioFixture.native.starts.length);
      faults.set(asset.asset_id, { kind });
      const eventId = await emit({ sourceId: asset.source_group });
      const code = { bytes: "AUDIO_INTEGRITY", http: "AUDIO_NETWORK", truncated: "AUDIO_BYTES" }[kind];
      await page.waitForFunction((code) => audioFixture.errors.some((e) => e.code === code), code);
      assert.equal(await page.evaluate(() => audioFixture.native.starts.length), before);
      assert.equal((await snapshot()).traces.some((t) => t.type === "started" && t.data.eventId === eventId), false);
      proof.push({ sourceId: asset.source_group, failure: kind, code, sourceStarted: false });
    }
    return { cases: proof };
  });

  await check("decoder rejection and a slow superseded jingle report/recover without a stale start", async () => {
    const sourceId = 2739;
    await page.evaluate(() => {
      const context = audioFixture.context;
      context.savedDecoder = context.decodeAudioData;
      context.decodeAudioData = () => Promise.reject(new DOMException("Injected decoder rejection on hash-verified input", "EncodingError"));
    });
    const failed = await emit({ sourceId });
    await page.waitForFunction(() => audioFixture.errors.some((error) => error.code === "AUDIO_DECODE"));
    assert.equal((await snapshot()).traces.some((t) => t.type === "started" && t.data.eventId === failed), false);
    await page.evaluate(() => {
      audioFixture.context.decodeAudioData = audioFixture.context.savedDecoder;
      delete audioFixture.context.savedDecoder;
    });
    const recovered = await emit({ sourceId });
    await waitEnded(recovered);
    const slow = manifest.assets.find((a) => a.kind === "jingle" && a.source_group === 30);
    faults.set(slow.asset_id, { kind: "delay", ms: 500 });
    const first = await emit({ kind: "jingle", sourceId: 30, payload: { committed: true, auxiliary: 60000 } });
    const waitUntil = Date.now() + 5000;
    while (!requests.some((r) => r.id === slow.asset_id) && Date.now() < waitUntil) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    assert.ok(requests.some((r) => r.id === slow.asset_id));
    const last = await emit({ kind: "jingle", sourceId: 33, payload: { committed: true, auxiliary: 0 } });
    await waitVoice(33);
    await page.waitForTimeout(600);
    assert.equal((await snapshot()).traces.some((t) => t.type === "started" && t.data.eventId === first), false);
    assert.equal((await snapshot()).voices.find((v) => v.kind === "jingle").eventId, last);
    return { decoderFailureInjected: true, hashVerifiedRetryPlayedNaturally: 2739, supersededSlowJingle: 30, winner: 33, staleStarts: 0 };
  });

  await check("initialization integrity/origin failures create no context and no fallback audio", async () => {
    const before = await page.evaluate(() => audioFixture.native.contexts.length);
    const unsafe = await page.evaluate(async () => {
      const { createAudio } = await import("/web/audio/index.ts");
      const errors = [];
      try {
        await createAudio({
          baseUrl: "https://invalid.example",
          url: () => "https://invalid.example/audio",
          image: async () => null, json: async () => ({}),
        }, (error) => errors.push(error.code));
        return { unexpectedSuccess: true };
      } catch (error) { return { code: error.code, errors }; }
    });
    assert.equal(unsafe.code, "AUDIO_ORIGIN");
    faults.set("assets/manifests/osrs/audio-runtime.json", { kind: "bytes" });
    const malformed = await page.evaluate(async () => {
      const { createAudio } = await import("/web/audio/index.ts");
      try {
        await createAudio({
          baseUrl: location.origin, url: (id) => `${location.origin}/asset/${encodeURIComponent(id)}`,
          image: async () => null, json: async () => ({}),
        }, () => undefined);
        return "unexpected-success";
      } catch (error) { return error.code; }
    });
    assert.equal(malformed, "AUDIO_INTEGRITY");
    assert.equal(await page.evaluate(() => audioFixture.native.contexts.length), before);
    return { externalOriginRejectedBeforeRequest: true, manifestCorruptionRejected: true, createdContexts: 0 };
  });

  await check("a missing background file is not retried on every snapshot; a real gesture retries the original", async () => {
    const id = "asset.source.osrs.cache2695.audio-runtime.music.0";
    faults.set(id, { kind: "http" });
    await emit({ kind: "music", sourceId: 0, payload: { mode: "area" } });
    await page.waitForFunction(() => audioFixture.errors.some(
      (error) => error.code === "AUDIO_NETWORK" && error.message.includes("audio-runtime.music.0"),
    ));
    const requested = requests.filter((request) => request.id === id).length;
    const starts = await page.evaluate(() => audioFixture.native.starts.length);
    await page.evaluate(() => {
      for (let i = 0; i < 50; i++) audioFixture.update(audioFixture.world);
    });
    await page.waitForTimeout(150);
    assert.equal(requests.filter((request) => request.id === id).length, requested);
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), starts);
    await trustedUnlock();
    await waitVoice(0, "music");
    await emit({ kind: "music", sourceId: 2, payload: { mode: "area" } });
    await waitVoice(2, "music");
    return { snapshotRetryStormRequests: 0, missingFileStartedSources: 0, trustedRetryPlayedOriginalGroup: 0 };
  });

  await check("a real silent-device sink is not treated as enabled playback, and resume timeout is explicit", async () => {
    assert.equal(await page.evaluate(() => typeof audioFixture.context.setSinkId), "function");
    await page.evaluate(() => audioFixture.context.setSinkId({ type: "none" }));
    await page.waitForFunction(() => audioFixture.snapshot().outputEnabled === false);
    assert.equal((await snapshot()).voices.length, 0);
    await page.click("#unlock");
    assert.equal((await page.evaluate(() => audioFixture.lastUnlock)).code, "AUDIO_OUTPUT_DISABLED");
    await page.evaluate(() => audioFixture.context.setSinkId(""));
    await page.waitForFunction(() => audioFixture.snapshot().outputEnabled);
    await trustedUnlock();
    await waitVoice(2, "music");
    await page.evaluate(async () => {
      await audioFixture.context.suspend();
      audioFixture.context.savedResume = audioFixture.context.resume;
      audioFixture.context.resume = () => new Promise(() => {});
    });
    await page.click("#unlock");
    const timeout = await page.evaluate(() => audioFixture.lastUnlock);
    assert.equal(timeout.success, false);
    assert.equal(timeout.code, "AUDIO_RESUME_TIMEOUT");
    assert.equal((await snapshot()).contextState, "suspended");
    await page.evaluate(() => {
      audioFixture.context.resume = audioFixture.context.savedResume;
      delete audioFixture.context.savedResume;
    });
    await trustedUnlock();
    await waitVoice(2, "music");
    return { actualSilentSinkRejected: true, realDefaultSinkRestored: true, timeoutFaultInjected: true, timeoutCode: timeout.code, actualRecoveredState: (await snapshot()).contextState };
  });

  await check("native fade steps run on the real graph and offline PCM, without a fabricated seamless music loop", async () => {
    const autumn = manifest.assets.find((a) => a.kind === "music" && a.source_group === 2);
    const nativeFade = nativeMusic.cases.find((c) => c.case === "original-wo-wp-music-fade-steps").observed
      .find((c) => c.volume === 255 && c.fade_cycles === 60 && c.direction === "in").native_volumes;
    await emit({
      kind: "music", sourceId: 2,
      payload: { mode: "single", unlocked: true, boundary: "native_duration",
        fadeOutCycles: 0, fadeInDelayCycles: 0, fadeInCycles: 60 },
    });

    await waitVoice(2, "music");
    await page.waitForFunction(() => audioFixture.snapshot().voices.filter((v) => v.kind === "music").length === 1);
    const actualVoice = (await snapshot()).voices.find((v) => v.kind === "music");
    assert.equal(actualVoice.loop, false);
    assert.equal(actualVoice.loopEnd, 0);
    const measured = [];
    for (const elapsed of [0.2,0.4,0.6,0.8,1.25]) {
      await page.waitForFunction((time) => audioFixture.context.currentTime >= time, actualVoice.when + elapsed);
      const state = await snapshot();
      const gainValue = state.voices.find((v) => v.id === actualVoice.id).gain;
      const index = Math.min(60, Math.floor((state.currentTime - actualVoice.when) / 0.02));
      assert.ok([index-1,index,index+1].some((i) => nativeFade[Math.max(0,Math.min(60,i))] / 128 === gainValue));
      measured.push({ elapsed: state.currentTime - actualVoice.when, nativeMixerLevel: gainValue * 128 });
    }
    const offline = await page.evaluate(async ({ frames, nativeFade }) => {
      const original = audioFixture.native.buffers.get(frames);
      const context = new OfflineAudioContext(2, 44100, 22050);
      const source = context.createBufferSource();
      const gain = context.createGain();
      source.buffer = original;
      for (let i=0;i<nativeFade.length;i++) gain.gain.setValueAtTime(nativeFade[i]/128,i*0.02);
      source.connect(gain);
      gain.connect(context.destination);
      source.start();
      const output = await context.startRendering();
      let maxError = 0, clips = 0, boundarySamples = 0;
      for (let channel = 0; channel < 2; channel++) {
        const samples = output.getChannelData(channel);
        const expected = original.getChannelData(channel);
        for (let i = 0; i < samples.length; i++) {
          const frame = Math.min(nativeFade.length-1,Math.floor(i/441));
          const level = nativeFade[frame];
          let error = Math.abs(samples[i] - Math.fround(expected[i]*level/128));
          // Every sample is checked. At the exact discrete boundary, floating
          // time-to-sample rounding may choose the immediately adjacent step.
          if (error > 0 && i % 441 === 0 && frame > 0) {
            const adjacent = Math.abs(samples[i] - Math.fround(expected[i]*nativeFade[frame-1]/128));
            if (adjacent < error) { error = adjacent; boundarySamples++; }
          }
          maxError = Math.max(maxError, error);
        }
        for (const value of samples) if (Math.abs(value) > 1) clips++;
      }
      source.disconnect();
      gain.disconnect();
      return { frames: output.length, sampleRate: output.sampleRate, channels: output.numberOfChannels,
        maxError, boundarySamples, maximumBoundaryRoundingMs: 1000 / 22050, extraClipSamples: clips };
    }, { frames: autumn.signal.frames, nativeFade });
    assert.ok(offline.maxError <= 0.000023, JSON.stringify(offline));
    assert.equal(offline.extraClipSamples, 0);
    const starts = await page.evaluate(() => audioFixture.native.starts.length);
    await page.evaluate(() => {
      const world = audioFixture.world;
      world.revision = String(BigInt(world.revision) + 1n);
      world.player.region = "region.osrs.12850";
      world.player.tile = { x: 3222, y: 3218, plane: 0 };
      world.entities = [];
      audioFixture.update(world);
    });
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length), starts);
    assert.equal((await snapshot()).voices.find((v) => v.kind === "music").sourceId, 2);
    return {
      actualNativeFadeCycles: 60, measured, nativeLoopFlag: false,
      manualModeSurvivesRegionChange: true, offline, replayTimerSeconds: 229 * 0.6,
    };
  });

  await check("new native publication/representation requirements fail explicitly instead of clipping or fabricating assets", async () => {
    await emit({ kind:"music",sourceId:-1,payload:{ mode:"area" } });
    const before = await page.evaluate(() => audioFixture.native.starts.length);
    for (const group of [40,54,58,64,65]) {
      await emit({ kind:"jingle",sourceId:group,payload:{ committed:true } });
      await page.waitForFunction((group) => audioFixture.errors.some(
        (e) => e.code==="AUDIO_NATIVE_GAIN_INPUT_REQUIRED" && e.detail?.sourceId===group),group);
    }
    assert.equal(await page.evaluate(() => audioFixture.native.starts.length),before);
    const fetches=requests.length;
    for (const group of [64,327,163,145]) await emit({ kind:"music",sourceId:group,payload:{ mode:"area" } });
    assert.equal(requests.length,fetches);
    const errors = await page.evaluate(() => audioFixture.errors.filter((e) => e.code==="AUDIO_SOURCE_MUSIC_ASSET_REQUIRED"));
    assert.deepEqual(errors.map((e) => e.detail.sourceGroup),[64,327,163,145]);
    await emit({ kind:"music",sourceId:2,payload:{ mode:"area",fadeOutCycles:0,fadeInDelayCycles:0,fadeInCycles:0 } });
    await waitVoice(2);
    return { native255RepresentationNeeded:[40,54,58,64,65],newOriginalMusicGroupsNeeded:[64,327,163,145],
      guardedSourceStarts:0,unpublishedAssetFetches:0,originalFilesChanged:0 };
  });

  if (!process.argv.includes("--quick")) {
    await check("full-length live playlist uses the native table44 timer and preserves the one-pass release", async () => {
      const clipBaseline = (await page.evaluate(() => audioFixture.stats())).clips;
      await emit({
        kind: "music", sourceId: 2,
        payload: { mode: "playlist", unlocked: true, boundary: "native_duration", playlist: "[2,76]",
          fadeOutCycles: 0, fadeInDelayCycles: 0, fadeInCycles: 0 },
      });
      await waitVoice(2, "music");
      await page.waitForFunction(() => audioFixture.snapshot().voices.filter((v) => v.kind === "music").length === 1);
      const original = (await snapshot()).voices.find((v) => v.kind === "music" && v.sourceId === 2);
      const autumn = manifest.assets.find((a) => a.kind === "music" && a.source_group === 2);
      const boundary = original.when + 229 * 0.6;
      await page.waitForFunction((boundary) => audioFixture.context.currentTime >= boundary + 0.2 &&
        audioFixture.snapshot().voices.some((v) => v.kind === "music" && v.sourceId === 76 && v.when <= audioFixture.context.currentTime),
      boundary, { timeout: 155_000 });
      const state = await snapshot();
      const harmony = state.voices.find((v) => v.kind === "music" && v.sourceId === 76);
      const ended = state.traces.find((t) => t.type === "ended" && t.data.eventId === original.eventId);
      assert.ok(ended?.data.natural);
      assert.equal(state.voices.filter((v) => v.kind === "music" && v.when <= state.currentTime).length, 1);
      const timingErrorMs = Math.abs(harmony.when - boundary) * 1000;
      assert.ok(timingErrorMs <= 20);
      const monitor = await page.evaluate(() => audioFixture.stats());
      assert.equal(monitor.clips - clipBaseline, 0);
      assert.equal(monitor.channels, 2);
      return {
        actualElapsedSourceSeconds: state.currentTime - original.when,
        from: 2, to: 76, expectedBoundary: boundary, actualScheduledBoundary: harmony.when,
        timingErrorMs, originalNaturallyEndedAt: ended.audioTime, simultaneousPlayingMusicSources: 1,
        sourceDurationTicks: 229, sourceReleaseTailPreserved: 22050,
        additionalClipSamples: monitor.clips - clipBaseline, monitor,
      };
    });
  }

  await check("real AudioContext closure is a device failure, followed by idempotent complete disposal", async () => {
    await page.evaluate(() => audioFixture.context.close());
    await page.waitForFunction(() => audioFixture.errors.some((e) => e.code === "AUDIO_CONTEXT_CLOSED"));
    assert.equal((await snapshot()).contextState, "closed");
    assert.equal((await snapshot()).voices.length, 0);
    await page.evaluate(async () => { await audioFixture.dispose(); await audioFixture.handle.dispose(); });
    const state = await snapshot();
    assert.equal(state.disposed, true);
    assert.equal(state.queueSize, 0);
    assert.equal(state.voices.length, 0);
    assert.equal(state.cache.pending, 0);
    assert.equal(state.cache.decodedBytes, 0);
    assert.equal(await page.evaluate(() => audioFixture.native.connections.size), 0);
    return { contextClosed: true, activeNodesAfterDispose: 0, queuedAfterDispose: 0, decodedCacheBytesAfterDispose: 0 };
  });

  report.nativePlayback = await page.evaluate(() => ({
    starts: audioFixture.native.starts.length, ends: audioFixture.native.ends.length,
    startedOnlyWhileRunning: audioFixture.native.starts.every((s) => s.state === "running"),
    endedSourceIds: audioFixture.native.ends.map((e) => e.id),
    gestures: audioFixture.native.gestures,
  }));
  report.observedRuntimeErrors = await page.evaluate(() => audioFixture.errors);
  assert.equal(report.failures.length, 0, report.failures.join("\n"));
  report.complete = true;
} catch (error) {
  report.failures.push(error.stack ?? String(error));
  if (page && !page.isClosed()) {
    report.lastState = await snapshot().catch(() => null);
    report.lastFixtureErrors = await page.evaluate(() => audioFixture.errors).catch(() => null);
  }
  process.exitCode = 1;
  console.error(error.stack ?? error);
} finally {
  if (page && !page.isClosed()) await page.evaluate(() => audioFixture.dispose()).catch(() => undefined);
  await context?.close();
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
  let changed = 0;
  for (const [path, hash] of originalsBefore) {
    if (digest(await readFile(path)) !== hash) changed++;
  }
  report.sourceFilesChanged = changed;
  if (changed !== 0) { report.complete = false; process.exitCode = 1; }
  report.implementationChangedDuringRun = [];
  for (const input of implementationFiles) {
    if (digest(await readFile(input.path)) !== input.sha256) report.implementationChangedDuringRun.push(input.path);
  }
  if (report.implementationChangedDuringRun.length > 0) {
    report.complete = false;
    process.exitCode = 1;
  }
  report.requestedPlayableIds = [...new Set(requests.filter((r) => sources.some((a) => (a.asset_id ?? a.id) === r.id)).map((r) => r.id))];
  report.finishedAt = new Date().toISOString();
  await writeFile(`${owned}/results.json`, `${JSON.stringify(report, null, 2)}\n`);
  await rm(run, { recursive: true, force: true });
  for (const name of await readdir("web/audio")) {
    if (!scratchBefore.has(name) && /^(?:\.?org\.chromium\.|playwright-)/.test(name)) {
      await rm(join("web/audio", name), { recursive: true, force: true });
    }
  }
  console.log(`${results.length} browser checks passed; original files changed: ${changed}; complete: ${report.complete}`);
}
