import { readFile, realpath } from "node:fs/promises";
import type { BrowserContext } from "playwright-core";
import { requireCondition } from "./config.ts";
import type { HarnessConfig } from "./config.ts";
import { inspectExecutable, checkExecutableVersion } from "./platform.ts";
import type { SupportedPlatform } from "./platform.ts";

export { checkExecutableVersion } from "./platform.ts";

export const graphicsProfiles = {
  "sparky-vulkan-x11": [
    "--enable-unsafe-webgpu",
    "--enable-features=Vulkan",
    "--use-angle=vulkan",
    "--enable-gpu",
    "--ignore-gpu-blocklist",
    "--ozone-platform=x11",
  ],
  "linux-webgpu-default-x11": [
    "--enable-unsafe-webgpu", "--enable-gpu", "--ignore-gpu-blocklist", "--ozone-platform=x11",
  ],
  "desktop-default": [],
  "mac-metal-default": [],
} as const;

export function checkSandboxArguments(args: string[]): void {
  requireCondition(!args.some((arg) => /^(--no-sandbox|--disable-.*sandbox|--disable-web-security|--single-process|--in-process-gpu|--no-zygote)(=|$)/.test(arg)),
    "Forbidden sandbox-disabling browser argument");
}

export function checkSandbox(text: string, args: string[]): void {
  checkSandboxArguments(args);
  requireCondition(/Layer 1 Sandbox\s+Namespace/i.test(text)
    && /PID namespaces\s+Yes/i.test(text) && /Network namespaces\s+Yes/i.test(text)
    && /Seccomp-BPF\s+sandbox\s+Yes/i.test(text),
    "Linux namespace and seccomp-BPF sandbox were not both verified");
}

export function sandboxEvidence(platform: SupportedPlatform, gpuSandboxed: unknown, linuxText: string | null) {
  if (platform === "linux") checkSandbox(linuxText ?? "", []);
  return {
    platform,
    mechanism: platform === "darwin" ? "macOS Seatbelt (Chromium), not the App Sandbox entitlement" : "Linux namespace/seccomp",
    sandboxRequested: true,
    namespaceAndSeccompVerified: platform === "linux" ? true : null,
    gpuProcessSandboxed: typeof gpuSandboxed === "boolean" ? gpuSandboxed : null,
    gpuEvidenceSource: "Chromium CDP optional GPU auxAttributes.sandboxed; not an independent OS attestation",
    diagnosticText: platform === "linux" ? linuxText : null,
    nativeAttestation: platform === "darwin" ? "unknown-owner-collection-required" : "renderer-namespace-seccomp-checked",
    securityCertification: "not-performed",
    limitations: platform === "darwin"
      ? ["No supported Node/CDP API establishes all renderer Seatbelt policies. No Linux diagnostic text is used. Collect owner-native evidence without disabling security."]
      : ["Renderer namespace/seccomp checks do not imply GPU-process isolation."],
  };
}

export function validateCandidateSandbox(config: HarnessConfig, evidence: ReturnType<typeof sandboxEvidence>): void {
  if (config.purpose !== "candidate" || config.mode !== "benchmark") return;
  if (evidence.platform === "linux") {
    requireCondition(evidence.gpuProcessSandboxed === true,
      "GPU process sandbox is not established for a Linux candidate benchmark; this profile remains observation-only");
  } else {
    requireCondition(evidence.gpuProcessSandboxed !== false,
      "Browser explicitly reports the Mac GPU process unsandboxed; preserve this failure and investigate without bypassing security");
    // Unknown Mac Seatbelt attestation is reported, not replaced by a Linux gate
    // or promoted to a security pass. Measurement does not certify M1/security.
  }
}

export async function executableMetadata(config: HarnessConfig) {
  const override = process.env[config.browser.executableEnv];
  const executable = config.browser.executablePath ?? override;
  requireCondition(executable, `Set ${config.browser.executableEnv} to the actual ${config.browser.product} executable`);
  if (override && config.browser.executablePath) {
    requireCondition(await realpath(override) === await realpath(config.browser.executablePath), "Executable environment override differs from the pinned original path");
  }
  const metadata = await inspectExecutable(executable, config.browser.product);
  checkExecutableVersion(metadata.versionOutput, config.browser.product, config.browser.expectedVersion);
  requireCondition(!config.browser.executableSha256 || metadata.sha256 === config.browser.executableSha256,
    "Browser executable digest changed; rediscover/reprobe and freeze new configuration before measurement");
  return metadata;
}

export async function browserMetadata(context: BrowserContext, config: HarnessConfig) {
  requireCondition(process.platform === "linux" || process.platform === "darwin", "Unsupported native browser platform");
  const browser = context.browser();
  requireCondition(browser, "No browser process");
  const session = await browser.newBrowserCDPSession();
  const version = await session.send("Browser.getVersion");
  const command = await session.send("Browser.getBrowserCommandLine");
  const system = await session.send("SystemInfo.getInfo");
  const processes = await session.send("SystemInfo.getProcessInfo");
  const linuxProcessIsolation = process.platform === "linux" ? await Promise.all(processes.processInfo
    .filter((p) => p.type === "GPU" || p.type === "renderer")
    .map(async (p) => {
      try {
        const status = await readFile(`/proc/${p.id}/status`, "utf8");
        return { type: p.type, pid: p.id, fields: status.split("\n").filter((line) => /^(NoNewPrivs|Seccomp|Seccomp_filters):/.test(line)) };
      } catch (error) {
        return { type: p.type, pid: p.id, unavailable: String(error) };
      }
    })) : [];
  const diagnostics = await context.newPage();
  let sandboxText: string | null = null;
  let userAgentData: {
    userAgent: string; brands: Array<{ brand: string; version: string }>;
    architecture: string | null; bitness: string | null; platformVersion: string | null;
  };
  try {
    const scheme = config.browser.product === "edge" ? "edge" : "chrome";
    await diagnostics.goto(`${scheme}://${process.platform === "linux" ? "sandbox" : "version"}`);
    checkSandboxArguments(command.arguments);
    if (process.platform === "linux") {
      sandboxText = await diagnostics.locator("body").innerText();
      checkSandbox(sandboxText, command.arguments);
    }
    userAgentData = await diagnostics.evaluate(async () => {
      const navigatorWithData = navigator as any;
      const values = await navigatorWithData.userAgentData?.getHighEntropyValues(["fullVersionList", "architecture", "bitness", "platformVersion"]);
      return {
        userAgent: navigator.userAgent, brands: values?.fullVersionList ?? [],
        architecture: values?.architecture ?? null, bitness: values?.bitness ?? null,
        platformVersion: values?.platformVersion ?? null,
      };
    });
  } finally {
    await diagnostics.close();
    await session.detach();
  }
  requireCondition(!/HeadlessChrome/.test(userAgentData.userAgent), "Headless browser is forbidden for capture");
  if (config.browser.product === "edge") {
    requireCondition(/ Edg\//.test(userAgentData.userAgent), "Real Edge user agent missing (Chrome cannot substitute)");
    requireCondition(userAgentData.brands.some((b) => /Microsoft Edge/.test(b.brand) && b.version === config.browser.expectedVersion),
      "Edge product/full version not confirmed by browser brand data");
  } else {
    requireCondition(!/ Edg\//.test(userAgentData.userAgent), "Edge cannot be labeled Chrome");
    requireCondition(browser.version() === config.browser.expectedVersion, "Running Chrome version differs from pin");
  }
  if (process.platform === "darwin") {
    requireCondition(!userAgentData.architecture || userAgentData.architecture === "arm",
      "Browser reports non-ARM execution; do not run the owner target under Rosetta");
  }
  const backend = system.gpu.auxAttributes as Record<string, unknown> | undefined;
  return {
    nativePlatform: process.platform,
    actualVersion: version,
    userAgentData,
    nativeExecution: process.platform === "darwin"
      ? { status: userAgentData.architecture === "arm" ? "browser-reported-arm" : "unknown", note: "An Intel Mac token in the ordinary UA does not imply Rosetta; use UA-CH and owner Activity Monitor Kind evidence" }
      : { status: "local-linux", note: "Not Mac or Edge evidence" },
    commandLine: command.arguments,
    gpu: system.gpu,
    sandbox: sandboxEvidence(process.platform, backend?.sandboxed, sandboxText),
    linuxProcessIsolation,
    processes: processes.processInfo.map(({ id, type }) => ({ pid: id, type })),
    browserPid: processes.processInfo.find((p) => p.type === "browser")?.id ?? null,
  };
}

export function validateBrowserBackend(
  gpu: { featureStatus?: Record<string, unknown>; auxAttributes?: Record<string, unknown> }, config: HarnessConfig,
): void {
  const features = gpu.featureStatus ?? {};
  requireCondition(features.gpu_compositing === "enabled", `Hardware GPU compositing unavailable: ${features.gpu_compositing}`);
  requireCondition(features.webgpu === "enabled", `Hardware WebGPU browser backend unavailable: ${features.webgpu}`);
  const backend = gpu.auxAttributes ?? {};
  requireCondition(!/swiftshader|llvmpipe|lavapipe|software/i.test([backend.glRenderer, backend.glVendor, backend.displayType].join(" ")),
    "Browser graphics backend indicates software rendering");
  if (config.browser.graphicsProfile === "sparky-vulkan-x11") {
    requireCondition(/Vulkan/i.test([backend.glRenderer, backend.displayType].join(" ")), "Pinned Vulkan/ANGLE backend not observed");
  }
  if (config.browser.graphicsProfile === "mac-metal-default") {
    requireCondition(/\bMetal\b|ANGLE_METAL/i.test([backend.glRenderer, backend.displayType].join(" ")),
      "Default Mac compositor Metal metadata is absent or different; do not relabel another backend as Metal");
  }
}
