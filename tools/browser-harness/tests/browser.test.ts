import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { rm, readFile } from "node:fs/promises";
import path from "node:path";
import { test } from "node:test";
import { loadConfig } from "../src/config.ts";
import { runHarness } from "../src/capture.ts";
import { startFixture } from "../src/fixture-server.ts";

const base = await loadConfig("config/fixture.json");

async function exercise(scenario: string) {
  const config = structuredClone(base);
  if (scenario !== "requires-input") config.contract.measurement.readinessTimeoutMs = 1500;
  const forbidden = scenario === "external-request" ? await startFixture(config) : null;
  const initialRequests = forbidden?.requestCount();
  const server = await startFixture(config, 0, forbidden?.origin);
  config.url = `${server.origin}/?case=${scenario}`;
  config.allowedOrigins = [server.origin];
  try {
    const result = await runHarness(config, { runId: `test-${scenario}-${randomUUID().slice(0, 8)}` });
    if (forbidden) assert.equal(forbidden.requestCount(), initialRequests, "An off-allowlist request reached the owned control server");
    return result;
  } finally {
    await server.close();
    await forbidden?.close();
  }
}

test("Chrome namespace/seccomp + real WebGPU fixture + keyboard/mouse + nonblank primary/resize capture", { timeout: 120_000 }, async () => {
  const result = await exercise("requires-input");
  assert.equal(result.ok, true, `${result.failures.join("\n")}\nArtifacts: ${result.directory}`);
  const report = result.report as any;
  assert.equal(report.status, "valid-measurement");
  assert.equal(report.m1Acceptance, "not-evaluated");
  assert.equal(report.baselineApproved, false);
  assert.equal(report.cleanup.browserExitVerified, true);
  assert.equal(report.browser.sandbox.namespaceAndSeccompVerified, true);
  assert.equal(report.browser.gpu.featureStatus.webgpu, "enabled");
  assert.equal(report.browser.gpu.featureStatus.gpu_compositing, "enabled");
  assert.equal(report.browser.commandLine.includes("--no-sandbox"), false);
  assert.equal(report.browser.commandLine.includes("--enable-unsafe-swiftshader"), false);
  assert.equal(report.desktopCapabilities.finePointer, true);
  assert.equal(report.desktopCapabilities.hover, true);
  assert.equal(report.desktopCapabilities.maxTouchPoints, 0);
  assert.ok(report.measurement.renderedFrames > 0);
  assert.ok(report.measurement.elapsedMs >= 2500);
  assert.equal(report.measurement.budgetDecision, "not-applicable-tool-fixture");
  assert.equal(report.measurement.gpuRenderPassDurationMs, null);
  assert.equal(report.queueCompletion.kind, "queue-completion-wall-time-after-window");
  assert.equal(report.captures.length, 4);
  assert.deepEqual(report.captures.map((c: any) => [c.viewport.width, c.viewport.height]), [[1920, 1080], [1920, 1080], [1280, 720], [2560, 1440]]);
  const events = JSON.parse(await readFile(path.join(result.directory, "events.json"), "utf8"));
  assert.equal(events.filter((e: any) => e.kind === "desktop-input").length, 3);
  console.log(`Retained TOOL FIXTURE ONLY artifacts: ${path.relative(process.cwd(), result.directory)}`);
});

for (const [scenario, expected] of [
  ["blank", /Blank\/nearly uniform screenshot/],
  ["missing", /Missing or unready rendered-frame instrumentation/],
  ["stale", /Stale or future rendered-frame counter/],
  ["raf-only", /counter exceeds observed GPU/],
  ["wrong-workload", /Wrong pinned identity: workloadId/],
  ["missing-assets", /Required loaded asset missing/],
  ["wrong-backend", /webgpu/],
  ["bad-metric", /NaN|number/],
  ["external-request", /blocked-url/],
  ["console-error", /console-error/],
  ["page-error", /pageerror/],
  ["http-error", /http-error/],
] as const) {
  test(`real browser rejects ${scenario}`, { timeout: 60_000 }, async () => {
    const result = await exercise(scenario);
    assert.equal(result.ok, false, `Unexpected success: ${result.directory}`);
    assert.match(result.failures.join("\n"), expected, `Artifacts: ${result.directory}`);
    assert.equal(result.report.m1Acceptance, "not-evaluated");
    const persisted = JSON.parse(await readFile(path.join(result.directory, "report.json"), "utf8"));
    assert.equal(persisted.status, "failed");
    if (process.env.HARNESS_KEEP_TEST_ARTIFACTS !== "1") await rm(result.directory, { recursive: true, force: true });
  });
}

test("an actual tool fixture cannot be captured as a source reference", { timeout: 60_000 }, async () => {
  const config = structuredClone(base);
  const server = await startFixture(config);
  config.url = `${server.origin}/`;
  config.allowedOrigins = [server.origin];
  config.purpose = "source-observation";
  config.mode = "capture";
  config.sourceObservation = { referenceBuild: "synthetic-negative-test", sourceSnapshot: "not-a-source-pack", provenance: "negative-test-only" };
  try {
    const result = await runHarness(config, { runId: `test-not-a-source-${randomUUID().slice(0, 8)}` });
    assert.equal(result.ok, false);
    assert.match(result.failures.join("\n"), /Tool fixture marker/);
    assert.equal(result.report.baselineApproved, false);
    if (process.env.HARNESS_KEEP_TEST_ARTIFACTS !== "1") await rm(result.directory, { recursive: true, force: true });
  } finally {
    await server.close();
  }
});

test("optional runtime binding passes only the predeclared audit contract, not scene identity or metrics", { timeout: 60_000 }, async () => {
  const result = await exercise("bind-run");
  assert.equal(result.ok, true, result.failures.join("\n"));
  assert.equal(result.report.contractBinding, "renderer-bindRun-audit-identity-only");
  assert.equal(result.report.m1Acceptance, "not-evaluated");
  await rm(result.directory, { recursive: true, force: true });
});
