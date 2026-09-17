import assert from "node:assert/strict";
import { test } from "node:test";
import { PNG } from "pngjs";
import { checkExecutableVersion, checkSandbox } from "../src/browser.ts";
import { canonicalJson, contractHash, isAllowedUrl, loadConfig, parseConfig, verifySourcePack } from "../src/config.ts";
import type { HarnessConfig } from "../src/config.ts";
import { distribution, expectedIdentity, inspectImage, summarizeFrames, validateGpu, validateSample, validateViewport } from "../src/metrics.ts";
import type { Sample } from "../src/metrics.ts";
import type { RenderFrame } from "../src/protocol.ts";

const fixture = await loadConfig("config/fixture.json");
const clone = () => structuredClone(fixture);
function sample(config = fixture): Sample {
  return {
    observedAtMs: 1001,
    viewport: { width: 1920, height: 1080, deviceScaleFactor: 1 },
    gpu: {
      secureContext: true, webgpuAvailable: true, errors: [],
      canvas: { width: 1920, height: 1048, cssWidth: 1920, cssHeight: 1048 },
      device: {
        id: 1, adapter: { vendor: "test-vendor", architecture: "test-architecture", device: "", description: "" },
        fallbackAdapter: false, adapterFeatures: ["timestamp-query"], features: [],
        submissions: 60, completedSubmissions: 60, lastCompletedAtMs: 1000,
        canvasAcquisitions: 60, canvasConfigurations: 1, lost: false,
      },
    },
    snapshot: {
      version: 1, clock: "performance.now", application: "harness-fixture", identity: expectedIdentity(config),
      ready: true, backend: "webgpu", viewport: { width: 1920, height: 1080, deviceScaleFactor: 1 },
      assets: config.contract.requiredAssets.map((a) => ({ ...a, loaded: true })),
      entities: { fixtureTriangles: 1 }, deviceEpoch: "one",
      renderedFrames: 60, nowMs: 1001, lastSubmittedAtMs: 999, lastCompletedAtMs: 1000, frames: [],
    },
  };
}
function next(previous: Sample): Sample {
  const s = structuredClone(previous);
  s.observedAtMs += 17;
  s.snapshot.nowMs += 17;
  s.snapshot.lastSubmittedAtMs += 17;
  s.snapshot.lastCompletedAtMs += 17;
  s.snapshot.renderedFrames++;
  s.gpu.device!.submissions++;
  s.gpu.device!.completedSubmissions++;
  s.gpu.device!.lastCompletedAtMs! += 17;
  s.gpu.device!.canvasAcquisitions++;
  s.snapshot.frames = [{
    sequence: s.snapshot.renderedFrames, submittedAtMs: s.snapshot.lastSubmittedAtMs,
    completedAtMs: s.snapshot.lastCompletedAtMs, drawCalls: 1, primitives: 1,
  }];
  return s;
}

test("valid pinned fixture config and monotonic submitted-frame sample", () => {
  const initial = validateSample(sample(), fixture);
  validateSample(next(initial), fixture, initial);
});

test("a pinned product artifact must match the dynamic renderer identity", () => {
  const config = clone(); config.contract.buildArtifactSha256 = "a".repeat(64);
  const current = sample(config);
  validateSample(current, config);
  delete current.snapshot.identity.buildArtifactSha256;
  assert.throws(() => validateSample(current, config), /buildArtifactSha256/);
  current.snapshot.identity.buildArtifactSha256 = "b".repeat(64);
  assert.throws(() => validateSample(current, config), /buildArtifactSha256/);
});

for (const url of [
  "https://example.com/", "http://example.com/", "http://localhost:4173/",
  "http://127.0.0.1:4174/", "http://127.0.0.1:4173.evil.test/",
  "file:///etc/passwd", "javascript:alert(1)", "http://user:password@127.0.0.1:4173/",
  "http://2130706433:4173/", "http://[::1]:4173/",
]) {
  test(`reject nonallowlisted target ${url.replace(/password/g, "synthetic")}`, () => {
    assert.equal(isAllowedUrl(url, fixture.allowedOrigins), false);
    const config = clone(); config.url = url;
    assert.throws(() => parseConfig(config));
  });
}
test("allow only explicit loopback HTTP/WebSocket origins", () => {
  assert.equal(isAllowedUrl(fixture.url, fixture.allowedOrigins), true);
  assert.equal(isAllowedUrl("ws://127.0.0.1:4173/session", fixture.allowedOrigins, true), true);
  assert.equal(isAllowedUrl("wss://127.0.0.1:4173/session", fixture.allowedOrigins, true), false);
  assert.throws(() => parseConfig({ ...clone(), allowedOrigins: ["http://127.0.0.1:*"] }));
  assert.throws(() => parseConfig({ ...clone(), allowedOrigins: ["http://example.com"] }));
});
test("reject wrong configured viewport, scale, resize range and omitted endpoints", () => {
  for (const mutate of [
    (c: HarnessConfig) => { c.contract.viewport.width = 1280; },
    (c: any) => { c.contract.viewport.deviceScaleFactor = 0.5; },
    (c: HarnessConfig) => { c.contract.resizeRange.max.width = 1600; },
    (c: HarnessConfig) => { c.contract.resizeChecks = [{ width: 1920, height: 1080 }, { width: 1920, height: 1080 }]; },
  ]) {
    const config = clone(); mutate(config); assert.throws(() => parseConfig(config));
  }
});
test("reject unknown browser switches and sandbox bypass options", () => {
  assert.throws(() => parseConfig({ ...clone(), browser: { ...fixture.browser, args: ["--no-sandbox"] } }));
});
test("a candidate needs an approved source pack and cannot use fixture identity", () => {
  const config = clone(); config.purpose = "candidate";
  assert.throws(() => parseConfig(config), /approved.*source pack/);
  config.contract.sourcePack = { id: "synthetic-unit-input", path: "tools/browser-harness/fixtures/triangle.wgsl", sha256: fixture.contract.requiredAssets[0].sha256, ownerApprovalRef: "synthetic-unit-test-not-approval" };
  assert.throws(() => parseConfig(config), /fixture cannot be a candidate/);
  config.contract.buildId = "synthetic-unit-client";
  assert.throws(() => parseConfig(config), /5s warmup and 60s measurement/);
  config.contract.measurement.warmupMs = 5000;
  config.contract.measurement.durationMs = 60_000;
  assert.throws(() => parseConfig(config), /hardware contract/);
});
test("source observations cannot masquerade as benchmarks", () => {
  const config = clone(); config.purpose = "source-observation";
  assert.throws(() => parseConfig(config), /Source observations/);
});
test("fixture configuration cannot claim a source pack", () => {
  const config = clone();
  config.contract.sourcePack = { id: "synthetic-unit-test", path: "not-read", sha256: "a".repeat(64), ownerApprovalRef: "not-approval" };
  assert.throws(() => parseConfig(config), /fixtures must not claim/);
});
test("source-pack bytes, not just an ID string, must match the pin", async () => {
  const config = clone();
  config.contract.sourcePack = { id: "unit-test-file-not-a-source-pack", path: "tools/browser-harness/fixtures/triangle.wgsl", sha256: fixture.contract.requiredAssets[0].sha256, ownerApprovalRef: "unit-test-not-approval" };
  await verifySourcePack(config);
  config.contract.sourcePack.sha256 = "0".repeat(64);
  await assert.rejects(verifySourcePack(config), /digest mismatch/);
  config.contract.sourcePack.path = "/not-allowed";
  await assert.rejects(verifySourcePack(config), /repository-relative/);
});
test("contract digest is stable by key ordering and changes with workload/budgets", () => {
  assert.equal(canonicalJson({ z: 1, a: { c: 2, b: 3 } }), canonicalJson({ a: { b: 3, c: 2 }, z: 1 }));
  const config = clone(); config.contract.sceneId += "-different";
  assert.notEqual(contractHash(config.contract), contractHash(fixture.contract));
  const altered = clone(); altered.contract.measurement.durationMs += 1;
  assert.notEqual(contractHash(altered.contract), contractHash(fixture.contract));
});
test("60 FPS cannot be configured down or supplied as a limiter-only metric", () => {
  const config: any = clone(); config.contract.measurement.minimumFps = 59;
  assert.throws(() => parseConfig(config));
  assert.throws(() => validateSample({ ...sample(), snapshot: { configuredFps: 60, rafCallbacks: 60 } } as any, fixture));
});
test("actual product and exact executable version are checked", () => {
  checkExecutableVersion("Google Chrome for Testing 153.0.8010.12", "chrome", "153.0.8010.12");
  checkExecutableVersion("Microsoft Edge 153.0.4234.32", "edge", "153.0.4234.32");
  assert.throws(() => checkExecutableVersion("Google Chrome for Testing 153.0.8010.12", "edge", "153.0.8010.12"), /real edge/);
  assert.throws(() => checkExecutableVersion("Chromium 153.0.8010.12", "chrome", "153.0.8010.12"), /real chrome/);
  assert.throws(() => checkExecutableVersion("Google Chrome 154.0.0.0", "chrome", "153.0.8010.12"), /pinned version/);
});
test("namespace/seccomp diagnostics fail closed", () => {
  const status = "Layer 1 Sandbox\tNamespace\nPID namespaces\tYes\nNetwork namespaces\tYes\nSeccomp-BPF sandbox\tYes";
  checkSandbox(status, ["--enable-gpu"]);
  for (const flag of ["--no-sandbox", "--disable-gpu-sandbox", "--disable-seccomp-filter-sandbox", "--single-process"]) {
    assert.throws(() => checkSandbox(status, [flag]), /Forbidden/);
  }
  assert.throws(() => checkSandbox(status.replace("Seccomp-BPF sandbox\tYes", "Seccomp-BPF sandbox\tNo"), []), /not both verified/);
});

const invalidSnapshots: Array<[string, (s: any) => void]> = [
  ["missing metrics", (s) => { delete s.snapshot; }],
  ["NaN timestamps", (s) => { s.snapshot.nowMs = NaN; }],
  ["infinite counter", (s) => { s.snapshot.renderedFrames = Infinity; }],
  ["fractional counter", (s) => { s.snapshot.renderedFrames = 2.5; }],
  ["zero rendered frames", (s) => { s.snapshot.renderedFrames = 0; }],
  ["unloaded scene", (s) => { s.snapshot.ready = false; }],
  ["wrong backend", (s) => { s.snapshot.backend = "webgl"; }],
  ["wrong workload", (s) => { s.snapshot.identity.workloadId = "empty-scene"; }],
  ["wrong source/contract pin", (s) => { s.snapshot.identity.benchmarkContractSha256 = "0".repeat(64); }],
  ["missing required assets", (s) => { s.snapshot.assets = []; }],
  ["unloaded asset", (s) => { s.snapshot.assets[0].loaded = false; }],
  ["wrong asset hash", (s) => { s.snapshot.assets[0].sha256 = "0".repeat(64); }],
  ["missing real workload entities", (s) => { s.snapshot.entities = {}; }],
  ["wrong viewport", (s) => { s.viewport.width = 1280; }],
  ["renderer wrong viewport", (s) => { s.snapshot.viewport.width = 1280; }],
  ["downscaled canvas", (s) => { s.gpu.canvas.width = 960; }],
  ["stale counter", (s) => { s.observedAtMs = 4000; s.snapshot.nowMs = 4000; }],
  ["future submitted timestamp", (s) => { s.snapshot.lastSubmittedAtMs = 2000; }],
  ["fake callback counter", (s) => { s.gpu.device.completedSubmissions = 1; }],
  ["submit-only counter before GPU completion", (s) => { s.gpu.device.completedSubmissions = 0; }],
  ["adapter-only with no configured canvas", (s) => { s.gpu.canvas = null; }],
  ["software GPU", (s) => { s.gpu.device.adapter.description = "SwiftShader"; }],
  ["fallback GPU", (s) => { s.gpu.device.fallbackAdapter = true; }],
  ["unknown fallback qualification", (s) => { s.gpu.device.fallbackAdapter = null; }],
  ["device loss", (s) => { s.gpu.device.lost = true; }],
  ["uncaptured GPU error", (s) => { s.gpu.errors.push("validation error"); }],
];
for (const [name, mutate] of invalidSnapshots) {
  test(`reject ${name}`, () => {
    const current = sample(); mutate(current);
    assert.throws(() => validateSample(current, fixture));
  });
}

test("reject nonmonotonic counts, missing frames, stale record clocks and device resets", () => {
  const previous = sample();
  for (const mutate of [
    (s: Sample) => { s.snapshot.renderedFrames = 1; },
    (s: Sample) => { s.snapshot.frames = []; },
    (s: Sample) => { s.snapshot.frames[0].submittedAtMs = 999; },
    (s: Sample) => { s.snapshot.frames[0].completedAtMs = 998; },
    (s: Sample) => { s.snapshot.frames[0].sequence = 62; },
    (s: Sample) => { s.snapshot.frames[0].drawCalls = 0; },
    (s: Sample) => { s.snapshot.frames[0].primitives = 0; },
    (s: Sample) => { s.snapshot.deviceEpoch = "reset"; },
    (s: Sample) => { s.gpu.device!.completedSubmissions = previous.gpu.device!.completedSubmissions; },
  ]) {
    const current = next(previous); mutate(current);
    assert.throws(() => validateSample(current, fixture, previous));
  }
});
test("GPU timestamps need enabled timestamp-query, not just adapter support", () => {
  const previous = sample(); const current = next(previous);
  current.snapshot.frames[0].gpuDurationMs = 1.5;
  assert.throws(() => validateSample(current, fixture, previous), /timestamp-query/);
  current.gpu.device!.features = ["timestamp-query"];
  validateSample(current, fixture, previous);
});
test("actual adapter identity and DPR must match pinned expectations", () => {
  const config = clone(); config.contract.hardware.expectedAdapter = { vendor: "different", architecture: "", device: "", description: "" };
  assert.throws(() => validateGpu(sample().gpu, config), /Wrong renderer adapter/);
  assert.throws(() => validateViewport({ width: 1920, height: 1080, deviceScaleFactor: 2 }, fixture.contract.viewport), /pixel ratio/);
});

function timing(fps = 60) {
  const config = clone(); config.purpose = "candidate";
  config.contract.measurement.durationMs = 60_000;
  const start = sample(); start.observedAtMs = 1000;
  const frames: RenderFrame[] = Array.from({ length: fps * 60 }, (_, i) => ({
    sequence: 61 + i, submittedAtMs: 999 + (i + 1) * 1000 / fps,
    completedAtMs: 1000 + (i + 1) * 1000 / fps, drawCalls: 1, primitives: 1,
  }));
  const end = sample(); end.observedAtMs = 61_000; end.snapshot.renderedFrames = 60 + frames.length;
  return { config, start, end, frames };
}
test("genuine 60 rendered FPS produces measured distributions, not a configured rate", () => {
  const { config, start, end, frames } = timing();
  const result = summarizeFrames(start, end, frames, config);
  assert.equal(result.renderedFps, 60);
  assert.equal(result.budgetDecision, "pass");
  assert.equal(result.gpuRenderPassDurationMs, null);
  assert.equal(result.gpuTimestampCoverage, 0);
  assert.equal(result.stalls.count, 0);
});
test("59 rendered FPS fails; blank/zero/short windows cannot be certified", () => {
  const { config, start, end, frames } = timing(59);
  assert.match(summarizeFrames(start, end, frames, config).budgetFailures.join(";"), /below 60/);
  assert.throws(() => summarizeFrames(start, end, [], config), /No rendered frames/);
  end.observedAtMs--;
  assert.throws(() => summarizeFrames(start, end, frames, config), /shorter/);
  assert.throws(() => distribution([]));
  assert.throws(() => distribution([Infinity]));
});
test("final unrendered tail counts toward maximum gaps and stalls", () => {
  const { config, start, end, frames } = timing();
  frames.splice(-30); end.snapshot.renderedFrames -= 30;
  const result = summarizeFrames(start, end, frames, config);
  assert.equal(result.trailingGapMs, 500);
  assert.equal(result.maxGapMs, 500);
  assert.equal(result.stalls.count, 1);
  assert.match(result.budgetFailures.join(";"), /maximum/);
});
test("fixture numbers never become a performance pass", () => {
  const { config, start, end, frames } = timing();
  config.purpose = "tool-fixture";
  assert.equal(summarizeFrames(start, end, frames, config).budgetDecision, "not-applicable-tool-fixture");
});

function image(mode: "black" | "white" | "transparent" | "tiny-mark" | "two-colors") {
  const png = new PNG({ width: 100, height: 100 });
  for (let i = 0; i < png.data.length; i += 4) {
    const color = mode === "white" || mode === "tiny-mark" ? 255 : mode === "two-colors" && i < png.data.length / 2 ? 220 : 0;
    png.data[i] = color;
    png.data[i + 1] = color;
    png.data[i + 2] = color;
    png.data[i + 3] = mode === "transparent" ? 0 : 255;
  }
  if (mode === "tiny-mark") png.data[0] = png.data[1] = png.data[2] = 0;
  return PNG.sync.write(png);
}
for (const mode of ["black", "white", "transparent", "tiny-mark"] as const) {
  test(`reject ${mode} screenshot even if a frame counter was reported`, () => {
    assert.throws(() => inspectImage(image(mode), fixture.contract.imageChecks), /Blank/);
  });
}
test("nonblank PNG detection is plumbing, not source fidelity", () => {
  assert.equal(inspectImage(image("two-colors"), fixture.contract.imageChecks).dominantColorFraction, 0.5);
});
