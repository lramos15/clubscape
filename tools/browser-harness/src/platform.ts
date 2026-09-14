import { execFile } from "node:child_process";
import { createReadStream } from "node:fs";
import { access, open, readFile, realpath } from "node:fs/promises";
import { createHash } from "node:crypto";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { canonicalJson, harnessRoot, requireCondition, sha256 } from "./config.ts";
import type { HarnessConfig } from "./config.ts";

export type SupportedPlatform = "linux" | "darwin";
export type BrowserProduct = "chrome" | "edge";
export interface CommandResult { code: number | null; stdout: string; stderr: string; error?: string }
export type CommandRunner = (file: string, args: string[]) => Promise<CommandResult>;

export const runCommand: CommandRunner = async (file, args) => {
  try {
    const result = await promisify(execFile)(file, args, { timeout: 30_000, maxBuffer: 4 * 1024 * 1024 });
    return { code: 0, stdout: result.stdout, stderr: result.stderr };
  } catch (error) {
    const failure = error as Error & { stdout?: string; stderr?: string; code?: string | number; killed?: boolean };
    return {
      code: typeof failure.code === "number" ? failure.code : null,
      stdout: failure.stdout ?? "", stderr: failure.stderr ?? "",
      error: typeof failure.code === "string" ? failure.code : failure.killed ? "command timed out" : "command failed",
    };
  }
};

export interface HostFacts {
  platform: SupportedPlatform;
  architecture: string;
  nodeVersion: string;
  osVersion: string | null;
  osBuild: string | null;
  kernel: string;
  model: string | null;
  chip: string | null;
  cpuCores: number;
  memoryBytes: number;
  arm64Hardware: boolean | null;
  appleSilicon: boolean | null;
  gpus: Array<{ model: string | null; vendor: string | null; metalSupport: string | null }>;
  issues: string[];
  sources: string[];
}

const object = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : {};
const string = (value: unknown): string | null => typeof value === "string" && value.length > 0 ? value : null;

export function projectMacDisplays(input: unknown): HostFacts["gpus"] {
  const items = object(input).SPDisplaysDataType;
  requireCondition(Array.isArray(items), "system_profiler did not return SPDisplaysDataType");
  return items.map((item) => {
    const gpu = object(item);
    return {
      model: string(gpu.sppci_model) ?? string(gpu.spdisplays_chipset_model),
      vendor: string(gpu.spdisplays_vendor),
      metalSupport: string(gpu.spdisplays_metal) ?? string(gpu.spdisplays_metalfamily),
    };
  });
}

export async function collectHostFacts(
  platform = process.platform,
  architecture = process.arch,
  runner: CommandRunner = runCommand,
): Promise<HostFacts> {
  requireCondition(platform === "linux" || platform === "darwin", "This handoff supports native macOS and the existing Linux harness");
  const facts: HostFacts = {
    platform, architecture, nodeVersion: process.version, osVersion: null, osBuild: null,
    kernel: os.release(), model: null, chip: null, cpuCores: os.cpus().length,
    memoryBytes: os.totalmem(), arm64Hardware: null, appleSilicon: null, gpus: [], issues: [], sources: [],
  };
  if (platform === "darwin") {
    const queries = [
      ["/usr/bin/sw_vers", "-productVersion"], ["/usr/bin/sw_vers", "-buildVersion"],
      ["/usr/sbin/sysctl", "-n", "hw.model"], ["/usr/sbin/sysctl", "-n", "machdep.cpu.brand_string"],
      ["/usr/sbin/sysctl", "-n", "hw.memsize"], ["/usr/sbin/sysctl", "-n", "hw.ncpu"],
      ["/usr/sbin/sysctl", "-n", "hw.optional.arm64"],
      ["/usr/sbin/system_profiler", "-json", "SPDisplaysDataType"],
    ];
    const results = await Promise.all(queries.map(([file, ...args]) => runner(file, args)));
    const text = (index: number) => {
      const result = results[index];
      facts.sources.push(queries[index].join(" "));
      if (result.code !== 0 || !result.stdout.trim()) {
        facts.issues.push(`Unavailable native query: ${queries[index].join(" ")} (${result.error ?? result.code})`);
        return null;
      }
      return result.stdout.trim();
    };
    facts.osVersion = text(0);
    facts.osBuild = text(1);
    facts.model = text(2);
    facts.chip = text(3);
    for (const [index, key] of [[4, "memoryBytes"], [5, "cpuCores"]] as const) {
      const value = text(index);
      if (value && Number.isSafeInteger(Number(value)) && Number(value) > 0) facts[key] = Number(value);
      else facts.issues.push(`No valid ${key} from sysctl; value is Node's local observation`);
    }
    const arm = text(6);
    facts.arm64Hardware = arm === "1" ? true : arm === "0" ? false : null;
    facts.appleSilicon = facts.chip && facts.arm64Hardware !== null
      ? /^Apple M\d/.test(facts.chip) && facts.arm64Hardware : null;
    const displays = text(7);
    if (displays) {
      try { facts.gpus = projectMacDisplays(JSON.parse(displays)); }
      catch { facts.issues.push("Native display inventory was not recognized; raw serial/UUID-bearing data was discarded"); }
    }
  } else {
    const release = await readFile("/etc/os-release", "utf8");
    facts.osVersion = release.match(/^PRETTY_NAME="([^"]+)"/m)?.[1] ?? null;
    const cpu = await runner("lscpu", ["--json"]);
    facts.sources.push("/etc/os-release", "lscpu --json", "Node os.cpus/os.totalmem");
    if (cpu.code === 0) {
      try {
        const rows = object(JSON.parse(cpu.stdout)).lscpu;
        if (Array.isArray(rows)) facts.chip = rows.map(object).filter((r) => r.field === "Model name:").map((r) => string(r.data)).filter(Boolean).join(" + ") || null;
      } catch { facts.issues.push("lscpu JSON unavailable"); }
    } else facts.issues.push("lscpu unavailable; CPU model not established");
  }
  return facts;
}

export function hostFingerprint(facts: HostFacts): string {
  const { nodeVersion: _, issues: __, sources: ___, ...stable } = facts;
  return sha256(canonicalJson(stable));
}

export function requireOwnerMac(facts: HostFacts): void {
  requireCondition(facts.platform === "darwin" && facts.architecture === "arm64",
    "Owner Mac collection must run with native ARM64 Node on macOS, not Rosetta or a Linux mock");
  requireCondition(facts.appleSilicon === true && facts.model && facts.chip && facts.osVersion && facts.osBuild,
    "Exact Apple M-series model/chip/macOS facts are missing; collect them on the owner's Mac");
}

export function assertNativeConfiguration(config: HarnessConfig, platform = process.platform, architecture = process.arch): void {
  requireCondition(platform === "linux" || platform === "darwin", "Unsupported native capture platform");
  requireCondition(!config.browser.platform || config.browser.platform === platform, "Configuration targets a different native OS");
  requireCondition(!config.browser.architecture || config.browser.architecture === architecture, "Configuration targets a different native architecture");
  if (platform === "darwin") {
    requireCondition(architecture === "arm64" && config.browser.graphicsProfile === "mac-metal-default",
      "M-series Mac capture requires native ARM64 Node and mac-metal-default; no Linux/Xvfb/Vulkan flags");
  } else {
    requireCondition(config.browser.graphicsProfile !== "mac-metal-default", "Mac default-Metal profile cannot be run on Linux");
  }
}

export function runtimeEnvironment(platform: NodeJS.Platform, runtime: string, inherited: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  const relative = path.relative(harnessRoot, runtime);
  if (platform === "darwin") {
    // Chromium's documented Mac-specific override avoids NSTemporaryDirectory's
    // external path and singleton socket limits, without moving the executable.
    return { ...inherited, TMPDIR: runtime, MAC_CHROMIUM_TMPDIR: relative };
  }
  return {
    ...inherited, TMPDIR: relative, HOME: runtime, XDG_RUNTIME_DIR: runtime,
    XDG_CACHE_HOME: path.join(runtime, "cache"), XDG_CONFIG_HOME: path.join(runtime, "config"),
  };
}

export function defaultExecutableCandidates(
  product: BrowserProduct, platform: NodeJS.Platform, home: string, cachedChrome?: string,
): string[] {
  if (platform === "darwin") {
    const name = product === "chrome" ? "Google Chrome" : "Microsoft Edge";
    return ["/Applications", path.join(home, "Applications")].map((base) => path.join(base, `${name}.app`, "Contents", "MacOS", name));
  }
  if (platform === "linux") {
    return product === "chrome"
      ? ["/usr/bin/google-chrome", "/usr/bin/google-chrome-stable", ...(cachedChrome ? [cachedChrome] : [])]
      : ["/usr/bin/microsoft-edge", "/usr/bin/microsoft-edge-stable", "/opt/microsoft/msedge/msedge"];
  }
  return [];
}

export function checkExecutableVersion(output: string, product: BrowserProduct, expected?: string): string {
  requireCondition(product === "chrome" ? /^Google Chrome(?: for Testing)? /i.test(output)
    : /^Microsoft Edge(?: (?:Beta|Dev|Canary))? /i.test(output), `Executable is not the declared real ${product} product`);
  const version = output.match(/\d+\.\d+\.\d+\.\d+/)?.[0];
  requireCondition(version, "Browser executable did not report a full four-part version");
  requireCondition(!expected || version === expected, "Browser executable version differs from pinned version");
  return version;
}

export function machOArchitectures(header: Buffer): string[] {
  requireCondition(header.length >= 8, "Truncated Mach-O header");
  const magic = header.readUInt32BE(0);
  const cpu = (value: number) => value === 0x0100000c ? "arm64" : value === 0x01000007 ? "x64" : `cpu-${value.toString(16)}`;
  if (magic === 0xcafebabe || magic === 0xcafebabf) {
    const count = header.readUInt32BE(4);
    const step = magic === 0xcafebabe ? 20 : 32;
    requireCondition(count > 0 && count <= 32 && 8 + count * step <= header.length, "Invalid universal Mach-O architecture table");
    return Array.from({ length: count }, (_, i) => cpu(header.readUInt32BE(8 + i * step)));
  }
  if (magic === 0xcffaedfe) return [cpu(header.readUInt32LE(4))];
  if (magic === 0xfeedfacf) return [cpu(header.readUInt32BE(4))];
  throw new Error("Executable is not a supported 64-bit Mach-O binary");
}

export interface ExecutableRecord {
  path: string;
  version: string;
  versionOutput: string;
  sha256: string;
  macBundle?: {
    path: string; identifier: string; executableName: string; shortVersion: string;
    architectures: string[]; signatureVerified: boolean; signer: string; teamIdentifier: string | null;
    meaning: string;
  };
}

export async function inspectExecutable(
  requested: string, product: BrowserProduct, platform = process.platform, runner: CommandRunner = runCommand,
): Promise<ExecutableRecord> {
  const resolved = await realpath(requested);
  let macBundle: ExecutableRecord["macBundle"];
  if (platform === "darwin") {
    const match = resolved.match(/^(.*\.app)\/Contents\/MacOS\/([^/]+)$/);
    requireCondition(match, "Use the original installed vendor .app/Contents/MacOS executable, not a copy or renamed binary");
    const bundlePath = match[1];
    const plist = path.join(bundlePath, "Contents", "Info.plist");
    const keys = ["CFBundleIdentifier", "CFBundleExecutable", "CFBundleShortVersionString"];
    const values = await Promise.all(keys.map((key) => runner("/usr/bin/plutil", ["-extract", key, "raw", "-o", "-", plist])));
    requireCondition(values.every((v) => v.code === 0 && v.stdout.trim()), "Cannot read original vendor app bundle identity with plutil");
    const [identifier, executableName, shortVersion] = values.map((v) => v.stdout.trim());
    const vendorPattern = product === "chrome" ? /^com\.google\.Chrome(?:\.(?:beta|dev|canary))?$/i : /^com\.microsoft\.edgemac(?:\.(?:beta|dev|canary))?$/i;
    requireCondition(vendorPattern.test(identifier) && executableName === path.basename(resolved), "Vendor bundle identifier/executable mismatch");
    const handle = await open(resolved, "r");
    let architectures: string[];
    try {
      const bytes = Buffer.alloc(4096);
      const { bytesRead } = await handle.read(bytes, 0, bytes.length, 0);
      architectures = machOArchitectures(bytes.subarray(0, bytesRead));
    } finally { await handle.close(); }
    requireCondition(architectures.includes("arm64"), "Installed browser has no native ARM64 slice; Rosetta is not the owner target");
    const requirement = `anchor apple generic and identifier "${identifier}"`;
    const verify = await runner("/usr/bin/codesign", ["--verify", "--deep", "--strict", "-R", requirement, bundlePath]);
    requireCondition(verify.code === 0, `Original app signature verification failed (${verify.error ?? verify.code}); no re-signing/Gatekeeper bypass is allowed`);
    const detail = await runner("/usr/bin/codesign", ["-dv", "--verbose=4", bundlePath]);
    requireCondition(detail.code === 0, "Cannot collect vendor code-signing identity");
    const lines = `${detail.stdout}\n${detail.stderr}`;
    const signer = lines.match(/^Authority=(Developer ID Application: .+)$/m)?.[1] ?? "";
    requireCondition(product === "chrome" ? /^Developer ID Application: Google (?:LLC|Inc\.) \(/.test(signer)
      : /^Developer ID Application: Microsoft Corporation \(/.test(signer), "Code signature is not the expected vendor Developer ID");
    macBundle = {
      path: bundlePath, identifier, executableName, shortVersion, architectures, signatureVerified: true,
      signer, teamIdentifier: lines.match(/^TeamIdentifier=(.+)$/m)?.[1] ?? null,
      meaning: "Original vendor bundle and native slice verification; NOT running-process Seatbelt attestation",
    };
  }
  const output = await runner(resolved, ["--version"]);
  requireCondition(output.code === 0, `Browser --version failed (${output.error ?? output.code})`);
  const versionOutput = output.stdout.trim();
  const version = checkExecutableVersion(versionOutput, product);
  if (macBundle) requireCondition(macBundle.shortVersion === version, "Bundle version differs from running vendor executable version");
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(resolved)) hash.update(chunk);
  return { path: resolved, version, versionOutput, sha256: hash.digest("hex"), ...(macBundle ? { macBundle } : {}) };
}

export async function discoverExecutable(product: BrowserProduct, candidates: string[]) {
  const attempts: Array<{ path: string; status: string; detail?: string }> = [];
  for (const candidate of [...new Set(candidates)]) {
    try { await access(candidate); }
    catch (error) {
      attempts.push({ path: candidate, status: "unavailable", detail: (error as NodeJS.ErrnoException).code });
      continue;
    }
    try {
      const executable = await inspectExecutable(candidate, product);
      return { product, status: "available" as const, executable, attempts };
    } catch (error) {
      attempts.push({ path: candidate, status: "unusable", detail: error instanceof Error ? error.message : String(error) });
    }
  }
  return { product, status: "unavailable" as const, executable: null, attempts };
}
