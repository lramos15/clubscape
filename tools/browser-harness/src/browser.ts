import { execFile } from "node:child_process";
import { createReadStream } from "node:fs";
import { readFile, realpath } from "node:fs/promises";
import { createHash } from "node:crypto";
import { promisify } from "node:util";
import type { BrowserContext } from "playwright-core";
import { requireCondition } from "./config.ts";
import type { HarnessConfig } from "./config.ts";

export const graphicsProfiles = {
  "sparky-vulkan-x11": [
    "--enable-unsafe-webgpu",
    "--enable-features=Vulkan",
    "--use-angle=vulkan",
    "--enable-gpu",
    "--ignore-gpu-blocklist",
    "--ozone-platform=x11",
  ],
  "desktop-default": [],
} as const;

export function checkExecutableVersion(output: string, product: "chrome" | "edge", expected: string): void {
  requireCondition(product === "chrome" ? /^Google Chrome(?: for Testing)? /i.test(output)
    : /^Microsoft Edge(?: (?:Beta|Dev|Canary))? /i.test(output), `Executable is not the declared real ${product} product`);
  requireCondition(output.match(/\d+\.\d+\.\d+\.\d+/)?.[0] === expected, "Browser executable version differs from pinned version");
}

export function checkSandbox(text: string, args: string[]): void {
  requireCondition(!args.some((arg) => /^(--no-sandbox|--disable-.*sandbox|--single-process|--in-process-gpu)(=|$)/.test(arg)),
    "Forbidden sandbox-disabling browser argument");
  requireCondition(/Layer 1 Sandbox\s+Namespace/i.test(text)
    && /PID namespaces\s+Yes/i.test(text) && /Network namespaces\s+Yes/i.test(text)
    && /Seccomp-BPF\s+sandbox\s+Yes/i.test(text),
    "Linux namespace and seccomp-BPF sandbox were not both verified");
}

export async function executableMetadata(config: HarnessConfig) {
  const executable = process.env[config.browser.executableEnv];
  requireCondition(executable, `Set ${config.browser.executableEnv} to the actual ${config.browser.product} executable`);
  const resolved = await realpath(executable);
  const { stdout } = await promisify(execFile)(resolved, ["--version"], { timeout: 15_000 });
  checkExecutableVersion(stdout.trim(), config.browser.product, config.browser.expectedVersion);
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(resolved)) hash.update(chunk);
  return { path: resolved, versionOutput: stdout.trim(), sha256: hash.digest("hex") };
}

export async function browserMetadata(context: BrowserContext, config: HarnessConfig) {
  requireCondition(process.platform === "linux", "Automated sandbox attestation currently supports Linux only; add native OS diagnostics before certification");
  const browser = context.browser();
  requireCondition(browser, "No browser process");
  const session = await browser.newBrowserCDPSession();
  const version = await session.send("Browser.getVersion");
  const command = await session.send("Browser.getBrowserCommandLine");
  const system = await session.send("SystemInfo.getInfo");
  const processes = await session.send("SystemInfo.getProcessInfo");
  const linuxProcessIsolation = await Promise.all(processes.processInfo
    .filter((p) => p.type === "GPU" || p.type === "renderer")
    .map(async (p) => {
      try {
        const status = await readFile(`/proc/${p.id}/status`, "utf8");
        return { type: p.type, pid: p.id, fields: status.split("\n").filter((line) => /^(NoNewPrivs|Seccomp|Seccomp_filters):/.test(line)) };
      } catch (error) {
        return { type: p.type, pid: p.id, unavailable: String(error) };
      }
    }));
  const diagnostics = await context.newPage();
  let sandboxText: string;
  let userAgentData: { userAgent: string; brands: Array<{ brand: string; version: string }> };
  try {
    await diagnostics.goto(config.browser.product === "edge" ? "edge://sandbox" : "chrome://sandbox");
    sandboxText = await diagnostics.locator("body").innerText();
    checkSandbox(sandboxText, command.arguments);
    userAgentData = await diagnostics.evaluate(async () => {
      const navigatorWithData = navigator as any;
      const values = await navigatorWithData.userAgentData?.getHighEntropyValues(["fullVersionList"]);
      return { userAgent: navigator.userAgent, brands: values?.fullVersionList ?? [] };
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
  const features = system.gpu.featureStatus as Record<string, string>;
  requireCondition(features.gpu_compositing === "enabled", `Hardware GPU compositing unavailable: ${features.gpu_compositing}`);
  requireCondition(features.webgpu === "enabled", `Hardware WebGPU browser backend unavailable: ${features.webgpu}`);
  const backend = system.gpu.auxAttributes as Record<string, unknown>;
  requireCondition(!/swiftshader|llvmpipe|lavapipe|software/i.test([backend.glRenderer, backend.glVendor, backend.displayType].join(" ")),
    "Browser graphics backend indicates software rendering");
  if (config.browser.graphicsProfile === "sparky-vulkan-x11") {
    requireCondition(/Vulkan/i.test(JSON.stringify(system.gpu.auxAttributes)), "Pinned Vulkan/ANGLE backend not observed");
  }
  return {
    actualVersion: version,
    userAgentData,
    commandLine: command.arguments,
    gpu: system.gpu,
    sandbox: { namespaceAndSeccompVerified: true, gpuProcessSandboxed: backend.sandboxed ?? null, diagnosticText: sandboxText },
    linuxProcessIsolation,
    browserPid: processes.processInfo.find((p) => p.type === "browser")?.id ?? null,
  };
}
