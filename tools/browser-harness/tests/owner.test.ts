import assert from "node:assert/strict";
import { test } from "node:test";
import { randomUUID } from "node:crypto";
import { mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import path from "node:path";
import { harnessRoot, loadConfig, parseConfig } from "../src/config.ts";
import { hostFingerprint } from "../src/platform.ts";
import type { HostFacts } from "../src/platform.ts";
import { prepareOwnerConfiguration, validateInventory } from "../src/owner-data.ts";
import { bundleRuns, outputPath, removeBundledRuns, writeOwnerJson } from "../src/owner-files.ts";

const base = await loadConfig("config/fixture.json");
function mockedInputs() {
  const host: HostFacts = {
    platform: "darwin", architecture: "arm64", nodeVersion: "unit-mock-only",
    osVersion: "99.0-unit-mock", osBuild: "UNIT-MOCK-BUILD", kernel: "UNIT-MOCK-NOT-MACOS",
    model: "UNIT-MOCK-MODEL", chip: "Apple M1 UNIT MOCK", cpuCores: 8, memoryBytes: 8589934592,
    arm64Hardware: true, appleSilicon: true, gpus: [{ model: "UNIT MOCK GPU", vendor: "unit", metalSupport: "unit" }],
    issues: ["UNIT MOCK ONLY; no Mac was run"], sources: ["unit-mock"],
  };
  const executable = {
    path: "/unit-mock/Google Chrome.app/Contents/MacOS/Google Chrome",
    version: "153.0.0.1", versionOutput: "Google Chrome 153.0.0.1", sha256: "a".repeat(64),
    macBundle: {
      path: "/unit-mock/Google Chrome.app", identifier: "com.google.Chrome", executableName: "Google Chrome",
      shortVersion: "153.0.0.1", architectures: ["arm64"], signatureVerified: true,
      signer: "UNIT MOCK", teamIdentifier: "UNIT MOCK", meaning: "UNIT MOCK ONLY",
    },
  };
  const inventory = {
    schemaVersion: 1, collectedAt: "UNIT MOCK ONLY", scope: "local-native-discovery-not-acceptance",
    host, hostFingerprint: hostFingerprint(host), m1Acceptance: "not-evaluated", securityCertification: "not-performed",
    browsers: {
      chrome: { product: "chrome", status: "available", executable, attempts: [] },
      edge: { product: "edge", status: "unavailable", executable: null, attempts: [] },
    },
  };
  const configuration = parseConfig({
    ...base, browser: { ...base.browser, platform: "darwin", architecture: "arm64", graphicsProfile: "mac-metal-default" },
  });
  const probe = {
    purpose: "tool-fixture", status: "valid-measurement", hostFingerprint: inventory.hostFingerprint,
    executable, configuration,
    browser: {
      nativePlatform: "darwin",
      gpu: { devices: [{ deviceString: "UNIT MOCK Metal GPU", driverVendor: "UNIT MOCK", driverVersion: "" }] },
      sandbox: { platform: "darwin", nativeAttestation: "unknown-owner-collection-required", gpuProcessSandboxed: null, securityCertification: "not-performed" },
    },
    initialRenderer: { gpu: { device: { adapter: { vendor: "unit-apple", architecture: "", device: "", description: "" }, fallbackAdapter: false } } },
    cleanup: { browserExitVerified: true, runtimeRemoved: true },
    m1Acceptance: "not-evaluated", baselineApproved: false,
  };
  const { hardware: _, ...contract } = structuredClone(base.contract);
  const productCase = {
    version: 1, mode: "benchmark", url: "http://127.0.0.1:4173/", allowedOrigins: ["http://127.0.0.1:4173"], captureCaseId: "UNIT-MOCK-CASE",
    contract: {
      ...contract, id: "UNIT-MOCK-CONTRACT", buildId: "UNIT-MOCK-PRODUCT", buildArtifactSha256: "b".repeat(64),
      sourcePack: { id: "UNIT-MOCK-NOT-APPROVAL", path: "unit-mock.json", sha256: "c".repeat(64), ownerApprovalRef: "UNIT-MOCK-NOT-APPROVAL" },
      measurement: { ...contract.measurement, durationMs: 60_000, warmupMs: 5000 },
    },
  };
  const owner = { hardware_direction: { authority: "owner", record: "UNIT MOCK ONLY: I will test on an M series mac", sandbox_disable_authorized: false } };
  return { inventory, probe, productCase, owner };
}
const references = { inventory: "UNIT-MOCK-INVENTORY", probe: "UNIT-MOCK-PROBE" };

test("UNIT MOCK ONLY: owner config derives real-input pins without inventing product/source approvals", () => {
  const { inventory, probe, productCase, owner } = mockedInputs();
  const config = prepareOwnerConfiguration(inventory, probe, productCase, "chrome", owner, references);
  assert.equal(config.browser.platform, "darwin");
  assert.equal(config.browser.graphicsProfile, "mac-metal-default");
  assert.equal(config.browser.executableSha256, inventory.browsers.chrome.executable.sha256);
  assert.equal(config.contract.buildArtifactSha256, productCase.contract.buildArtifactSha256);
  assert.equal(config.contract.ownerApprovalRef, null);
  assert.equal(config.contract.sourcePack!.ownerApprovalRef, "UNIT-MOCK-NOT-APPROVAL");
  assert.match(config.contract.hardware.ownerApprovalRef!, /hardware_direction/);
  assert.equal(probe.browser.sandbox.nativeAttestation, "unknown-owner-collection-required");
  assert.deepEqual(config.contract.setupActions, productCase.contract.setupActions);
});

test("UNIT MOCK ONLY: missing source approval/product artifact and wrong loopback remain rejected", () => {
  const input = mockedInputs();
  const missingSource = { ...input.productCase, contract: { ...input.productCase.contract, sourcePack: null } };
  assert.throws(() => prepareOwnerConfiguration(input.inventory, input.probe, missingSource, "chrome", input.owner, references), /source pack/);
  const missingArtifact = { ...input.productCase, contract: { ...input.productCase.contract, buildArtifactSha256: undefined } };
  assert.throws(() => prepareOwnerConfiguration(input.inventory, input.probe, missingArtifact, "chrome", input.owner, references));
  assert.throws(() => prepareOwnerConfiguration(input.inventory, input.probe, { ...input.productCase, url: "https://remote.example/" }, "chrome", input.owner, references), /allowlisted/);
});

test("UNIT MOCK ONLY: Linux, wrong-browser, changed-binary and unsandboxed probes cannot become Mac evidence", () => {
  const input = mockedInputs();
  for (const mutate of [
    (p: any) => { p.browser.nativePlatform = "linux"; },
    (p: any) => { p.configuration.browser.product = "edge"; },
    (p: any) => { p.configuration.browser.graphicsProfile = "sparky-vulkan-x11"; },
    (p: any) => { p.executable.sha256 = "d".repeat(64); },
    (p: any) => { p.hostFingerprint = "e".repeat(64); },
    (p: any) => { p.browser.sandbox.gpuProcessSandboxed = false; },
  ]) {
    const probe = structuredClone(input.probe); mutate(probe);
    assert.throws(() => prepareOwnerConfiguration(input.inventory, probe, input.productCase, "chrome", input.owner, references));
  }
  assert.throws(() => prepareOwnerConfiguration(input.inventory, input.probe, input.productCase, "edge", input.owner, references), /No real edge/);
  assert.throws(() => validateInventory({ ...input.inventory, hostFingerprint: "0".repeat(64) }), /fingerprint mismatch/);
});

test("owner output cannot escape or silently overwrite existing inputs", async () => {
  for (const name of ["../outside.json", "/absolute.json", "owner-runs/../outside.json", "owner-runs"]) await assert.rejects(outputPath(name));
  const id = `unit-owner-${randomUUID()}`;
  const file = `owner-runs/${id}/inventory.json`;
  try {
    await writeOwnerJson(file, { purpose: "UNIT MOCK ONLY" });
    await assert.rejects(writeOwnerJson(file, {}), /EEXIST/);
  } finally { await rm(path.join(harnessRoot, "owner-runs", id), { recursive: true, force: true }); }
});

test("owner output rejects symlinked parent directories", async () => {
  const id = `unit-symlink-${randomUUID()}`;
  const parent = path.join(harnessRoot, "owner-runs", id);
  const target = path.join(harnessRoot, ".runtime", id);
  await mkdir(path.dirname(parent), { recursive: true }); await mkdir(target, { recursive: true });
  await symlink(target, parent);
  try { await assert.rejects(writeOwnerJson(`owner-runs/${id}/file.json`, {}), /symlink/); }
  finally { await rm(parent, { force: true }); await rm(target, { recursive: true, force: true }); }
});

test("bundles preserve failed observations and verify bytes before targeted raw-run cleanup", async () => {
  const id = `unit-bundle-${randomUUID()}`;
  const raw = path.join(harnessRoot, "runs", id);
  const bundle = `owner-runs/${id}`;
  await mkdir(raw, { recursive: true });
  await writeFile(path.join(raw, "report.json"), JSON.stringify({
    schemaVersion: 1, runId: id, purpose: "tool-fixture", status: "failed", m1Acceptance: "not-evaluated",
    host: { platform: "linux" }, cleanup: { browserPid: null, runtimeRemoved: true },
  }));
  await writeFile(path.join(raw, "events.json"), '[{"kind":"unit-only","message":"intentional failure"}]');
  try {
    const result = await bundleRuns([id], bundle);
    const manifestFile = path.join(result.directory, "manifest.json");
    const manifest = JSON.parse(await readFile(manifestFile, "utf8"));
    assert.equal(manifest.observations[0].status, "failed");
    assert.equal(manifest.observations[0].platform, "linux");
    assert.equal(manifest.m1Acceptance, "not-evaluated");
    assert.equal(manifest.baselineApproved, false);
    await writeFile(path.join(raw, "events.json"), "changed");
    await assert.rejects(removeBundledRuns(manifestFile), /differs from its archive/);
    await writeFile(path.join(raw, "events.json"), '[{"kind":"unit-only","message":"intentional failure"}]');
    assert.equal((await removeBundledRuns(manifestFile)).removed, 1);
    await assert.rejects(readFile(path.join(raw, "report.json")), /ENOENT/);
  } finally {
    await rm(raw, { recursive: true, force: true });
    await rm(path.join(harnessRoot, bundle), { recursive: true, force: true });
  }
});
