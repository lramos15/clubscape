import assert from "node:assert/strict";
import { test } from "node:test";
import { randomUUID } from "node:crypto";
import { mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import {
  assertNativeConfiguration, collectHostFacts, defaultExecutableCandidates, hostFingerprint,
  inspectExecutable, machOArchitectures, projectMacDisplays, requireOwnerMac, runtimeEnvironment,
} from "../src/platform.ts";
import type { CommandRunner, HostFacts } from "../src/platform.ts";
import { checkSandboxArguments, sandboxEvidence, validateBrowserBackend, validateCandidateSandbox, graphicsProfiles } from "../src/browser.ts";
import { harnessRoot, loadConfig, parseConfig } from "../src/config.ts";
import { expectedIdentity } from "../src/metrics.ts";

const base = await loadConfig("config/fixture.json");
const mac = () => parseConfig({
  ...structuredClone(base),
  browser: { ...base.browser, platform: "darwin", architecture: "arm64", graphicsProfile: "mac-metal-default" },
});

test("UNIT MOCK ONLY: native Mac configuration cannot be executed as Linux/Rosetta", () => {
  const config = mac();
  assertNativeConfiguration(config, "darwin", "arm64");
  assert.throws(() => assertNativeConfiguration(config, "linux", "arm64"), /different native OS/);
  assert.throws(() => assertNativeConfiguration(config, "darwin", "x64"), /different native architecture/);
  assert.throws(() => assertNativeConfiguration(base, "darwin", "arm64"), /no Linux/);
  assert.throws(() => parseConfig({ ...mac(), browser: { ...mac().browser, graphicsProfile: "sparky-vulkan-x11" } }), /not Linux/);
  assert.deepEqual(graphicsProfiles["mac-metal-default"], []);
});

test("UNIT MOCK ONLY: Mac sandbox evidence never accepts Linux strings as attestation", () => {
  const linux = "Layer 1 Sandbox\tNamespace\nPID namespaces\tYes\nNetwork namespaces\tYes\nSeccomp-BPF sandbox\tYes";
  const evidence = sandboxEvidence("darwin", undefined, linux);
  assert.equal(evidence.namespaceAndSeccompVerified, null);
  assert.equal(evidence.diagnosticText, null);
  assert.equal(evidence.gpuProcessSandboxed, null);
  assert.equal(evidence.nativeAttestation, "unknown-owner-collection-required");
  assert.equal(evidence.securityCertification, "not-performed");
  const candidate = { ...mac(), purpose: "candidate" as const };
  validateCandidateSandbox(candidate, evidence);
  assert.throws(() => validateCandidateSandbox(candidate, sandboxEvidence("darwin", false, null)), /unsandboxed/);
  assert.throws(() => validateCandidateSandbox(candidate, sandboxEvidence("linux", false, linux)), /Linux candidate/);
  assert.throws(() => validateCandidateSandbox(candidate, sandboxEvidence("linux", null, linux)), /Linux candidate/);
});

test("sandbox-bypassing flags remain rejected on every platform", () => {
  for (const flag of ["--no-sandbox", "--disable-gpu-sandbox", "--disable-seatbelt-sandbox", "--disable-web-security", "--single-process", "--no-zygote"]) {
    assert.throws(() => checkSandboxArguments([flag]), /Forbidden/);
  }
  checkSandboxArguments(["--enable-automation"]);
});

test("UNIT MOCK ONLY: Metal metadata is observed, not assumed from macOS or vendor", () => {
  const hardware = { featureStatus: { webgpu: "enabled", gpu_compositing: "enabled" } };
  validateBrowserBackend({ ...hardware, auxAttributes: { displayType: "ANGLE_METAL", glRenderer: "UNIT MOCK Apple ANGLE Metal Renderer" } }, mac());
  assert.throws(() => validateBrowserBackend({ ...hardware, auxAttributes: { displayType: "ANGLE_VULKAN", glRenderer: "Apple" } }, mac()), /Metal metadata/);
  assert.throws(() => validateBrowserBackend({ ...hardware, auxAttributes: { displayType: "ANGLE_METAL", glRenderer: "SwiftShader" } }, mac()), /software rendering/);
  assert.throws(() => validateBrowserBackend({}, mac()), /unavailable/);
});

test("UNIT MOCK ONLY: macOS environment uses Chromium's documented Mac override", () => {
  const runtime = path.join(harnessRoot, ".runtime", "unit-mock");
  const env = runtimeEnvironment("darwin", runtime, { HOME: "/unit-mock-owner", DISPLAY: "not-used-by-native-Mac" });
  assert.equal(env.HOME, "/unit-mock-owner");
  assert.equal(env.TMPDIR, runtime);
  assert.equal(env.MAC_CHROMIUM_TMPDIR, path.relative(harnessRoot, runtime));
  assert.equal(env.XDG_RUNTIME_DIR, undefined);
  const linux = runtimeEnvironment("linux", runtime, {});
  assert.equal(linux.HOME, runtime);
  assert.equal(linux.MAC_CHROMIUM_TMPDIR, undefined);
});

test("Mac discovery checks original standard vendor paths, never renamed Chrome for Edge", () => {
  assert.deepEqual(defaultExecutableCandidates("chrome", "darwin", "/unit-mock-home"), [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/unit-mock-home/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  ]);
  assert.deepEqual(defaultExecutableCandidates("edge", "darwin", "/unit-mock-home"), [
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/unit-mock-home/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  ]);
});

test("UNIT MOCK ONLY: Mach-O thin/universal ARM64 slices are parsed without lipo/installers", () => {
  const thin = Buffer.alloc(32); thin.writeUInt32BE(0xcffaedfe); thin.writeUInt32LE(0x0100000c, 4);
  assert.deepEqual(machOArchitectures(thin), ["arm64"]);
  const universal = Buffer.alloc(48);
  universal.writeUInt32BE(0xcafebabe); universal.writeUInt32BE(2, 4);
  universal.writeUInt32BE(0x01000007, 8); universal.writeUInt32BE(0x0100000c, 28);
  assert.deepEqual(machOArchitectures(universal), ["x64", "arm64"]);
  assert.throws(() => machOArchitectures(Buffer.from("not-mach-o")), /Mach-O/);
  universal.writeUInt32BE(1000, 4);
  assert.throws(() => machOArchitectures(universal), /architecture table/);
});

test("UNIT MOCK ONLY: original Mac bundle validation and codesign branching, not native attestation", async () => {
  const root = path.join(harnessRoot, ".runtime", `unit-mac-bundle-${randomUUID()}`);
  const file = path.join(root, "Google Chrome.app", "Contents", "MacOS", "Google Chrome");
  await mkdir(path.dirname(file), { recursive: true });
  const header = Buffer.alloc(32); header.writeUInt32BE(0xcffaedfe); header.writeUInt32LE(0x0100000c, 4);
  await writeFile(file, header);
  const commands: string[][] = [];
  const runner: CommandRunner = async (command, args) => {
    commands.push([command, ...args]);
    const ok = (stdout = "", stderr = "") => ({ code: 0, stdout, stderr });
    if (command.endsWith("plutil")) {
      return ok(({ CFBundleIdentifier: "com.google.Chrome", CFBundleExecutable: "Google Chrome", CFBundleShortVersionString: "153.0.0.1" } as Record<string, string>)[args[1]]);
    }
    if (command.endsWith("codesign")) {
      return args[0] === "-dv" ? ok("", "Authority=Developer ID Application: Google LLC (UNIT-MOCK)\nTeamIdentifier=UNIT-MOCK\n") : ok();
    }
    assert.equal(command, file); assert.deepEqual(args, ["--version"]);
    return ok("Google Chrome 153.0.0.1");
  };
  try {
    const executable = await inspectExecutable(file, "chrome", "darwin", runner);
    assert.equal(executable.macBundle?.signatureVerified, true);
    assert.match(executable.macBundle!.meaning, /NOT.*Seatbelt/);
    assert.ok(commands.some((c) => c.includes("anchor apple generic and identifier \"com.google.Chrome\"")));
    assert.ok(commands.every((c) => !c.includes("--no-sandbox")));
    await assert.rejects(inspectExecutable(file, "edge", "darwin", runner), /bundle identifier/);
    const failure: CommandRunner = async (command, args) => command.endsWith("codesign") ? { code: 1, stdout: "", stderr: "UNIT MOCK signature failure" } : runner(command, args);
    await assert.rejects(inspectExecutable(file, "chrome", "darwin", failure), /signature verification failed/);
  } finally { await rm(root, { recursive: true, force: true }); }
});

test("native inventory projection excludes serials, UUIDs, display identities and private fields", () => {
  const output = projectMacDisplays({
    secret: "private", SPDisplaysDataType: [{
      sppci_model: "UNIT MOCK Apple M1", spdisplays_vendor: "UNIT MOCK Apple", spdisplays_metal: "UNIT MOCK Metal",
      serial_number: "SECRET-SERIAL", hardware_uuid: "SECRET-UUID",
      _items: [{ display_serial: "SECRET-DISPLAY", apple_id: "SECRET-ACCOUNT" }],
    }],
  });
  assert.deepEqual(output, [{ model: "UNIT MOCK Apple M1", vendor: "UNIT MOCK Apple", metalSupport: "UNIT MOCK Metal" }]);
  assert.equal(JSON.stringify(output).includes("SECRET"), false);
});

test("UNIT MOCK ONLY: exact Mac facts use bounded native commands without broad system dumps", async () => {
  const outputs: Record<string, string> = {
    "-productVersion": "99.0-unit-mock", "-buildVersion": "UNIT-MOCK-BUILD", "hw.model": "UNIT-MOCK-MODEL",
    "machdep.cpu.brand_string": "Apple M1 UNIT MOCK", "hw.memsize": "8589934592", "hw.ncpu": "8", "hw.optional.arm64": "1",
  };
  const runner: CommandRunner = async (file, args) => {
    assert.ok(!args.includes("SPHardwareDataType") && !args.includes("SPSoftwareDataType"));
    if (file.endsWith("system_profiler")) return { code: 0, stdout: '{"SPDisplaysDataType":[]}', stderr: "" };
    return { code: 0, stdout: outputs[args[args.length - 1]], stderr: "" };
  };
  const facts = await collectHostFacts("darwin", "arm64", runner);
  assert.equal(facts.chip, "Apple M1 UNIT MOCK");
  assert.equal(facts.memoryBytes, 8589934592);
  requireOwnerMac(facts);
  assert.throws(() => requireOwnerMac({ ...facts, architecture: "x64" }), /Rosetta/);
  assert.throws(() => requireOwnerMac({ ...facts, chip: null }), /missing/);
  assert.notEqual(hostFingerprint(facts), hostFingerprint({ ...facts, osBuild: "changed" }));
  assert.equal(hostFingerprint(facts), hostFingerprint({ ...facts, sources: ["other text"], issues: ["unit-only"] }));
});

test("Mac product cases require deployed artifact and original executable/host pins", () => {
  const config = mac(); config.purpose = "candidate"; config.contract.buildId = "unit-product";
  assert.throws(() => parseConfig(config), /artifact SHA/);
  config.contract.buildArtifactSha256 = "b".repeat(64);
  assert.throws(() => parseConfig(config), /executable and host pins/);
  assert.equal(expectedIdentity(config).buildArtifactSha256, "b".repeat(64));
});

test("benchmark ownership is optional metadata, not an extra source-pack approval gate", () => {
  const config = structuredClone(base);
  config.purpose = "candidate";
  config.contract.buildId = "UNIT-MOCK-PRODUCT";
  config.contract.sourcePack = { id: "UNIT-MOCK-NOT-APPROVAL", path: "unit-mock.json", sha256: "a".repeat(64), ownerApprovalRef: "UNIT-MOCK-NOT-APPROVAL" };
  config.contract.measurement.warmupMs = 5000; config.contract.measurement.durationMs = 60_000;
  config.contract.hardware.representativeIntegratedGraphics = true;
  config.contract.hardware.ownerApprovalRef = "UNIT-MOCK-HARDWARE-DIRECTION";
  config.contract.hardware.expectedAdapter = { vendor: "unit", architecture: "", device: "", description: "" };
  config.contract.ownerApprovalRef = null;
  assert.equal(parseConfig(config).contract.ownerApprovalRef, null);
  config.contract.sourcePack = null;
  assert.throws(() => parseConfig(config), /approved.*source pack/);
});
