import os from "node:os";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { createRequire } from "node:module";
import { mkdir, readFile, readdir, realpath, rm, writeFile } from "node:fs/promises";
import { setTimeout as delay } from "node:timers/promises";
import { chromium } from "playwright-core";
import type { BrowserContext, Page } from "playwright-core";
import {
  canonicalJson, contractHash, harnessRoot, isAllowedUrl, parseConfig, requireCondition, sha256, verifySourcePack,
} from "./config.ts";
import type { HarnessConfig, Viewport } from "./config.ts";
import { browserMetadata, executableMetadata, graphicsProfiles, validateBrowserBackend, validateCandidateSandbox } from "./browser.ts";
import { assertNativeConfiguration, collectHostFacts, hostFingerprint, requireOwnerMac, runtimeEnvironment } from "./platform.ts";
import { installGpuObserver } from "./observer.ts";
import { inspectImage, summarizeFrames, validateGpu, validateSample, validateViewport } from "./metrics.ts";
import type { Sample } from "./metrics.ts";
import type { RenderFrame } from "./protocol.ts";

type LogEntry = { at: string; kind: string; message: string; url?: string };
const timestamp = () => new Date().toISOString();

async function bounded<T>(promise: Promise<T>, label: string, milliseconds = 15_000): Promise<T> {
  let timer: NodeJS.Timeout | undefined;
  try {
    return await Promise.race([promise, new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new Error(`${label} timed out after ${milliseconds}ms`)), milliseconds);
    })]);
  } finally {
    clearTimeout(timer);
  }
}

async function samplePage(page: Page, config: HarnessConfig, afterFrame: number | null): Promise<Sample> {
  return bounded(page.evaluate(({ selector, after }) => {
    const host = window as any;
    if (!host.__clubscapeBenchmarkV1 || typeof host.__clubscapeBenchmarkV1.read !== "function") {
      throw new Error("Missing window.__clubscapeBenchmarkV1.read rendered-frame instrumentation");
    }
    const snapshot = host.__clubscapeBenchmarkV1.read(after);
    if (snapshot && typeof snapshot.then === "function") throw new Error("Instrumentation read() must be synchronous");
    return {
      snapshot,
      observedAtMs: performance.now(),
      viewport: { width: innerWidth, height: innerHeight, deviceScaleFactor: devicePixelRatio },
      gpu: host.__clubscapeHarnessObserverV1.read(selector),
    };
  }, { selector: config.contract.surfaceSelector, after: afterFrame }), "Renderer sample", 5000);
}

async function readViewport(page: Page): Promise<Viewport> {
  return page.evaluate(() => ({ width: innerWidth, height: innerHeight, deviceScaleFactor: devicePixelRatio }));
}

async function saveCapture(page: Page, config: HarnessConfig, directory: string, label: string, viewport: Viewport) {
  validateViewport(await readViewport(page), viewport);
  const surface = page.locator(config.contract.surfaceSelector);
  requireCondition(await surface.count() === 1, "Capture surface selector must match exactly one element");
  const box = await surface.boundingBox();
  requireCondition(box && box.width * box.height >= viewport.width * viewport.height * config.contract.imageChecks.minSurfaceAreaFraction
    && box.x >= 0 && box.y >= 0 && box.x + box.width <= viewport.width + 0.5
    && box.y + box.height <= viewport.height + 0.5, "Capture surface is missing, too small, or outside viewport");
  const prefix = `${config.purpose}-${label}`;
  const startedAt = timestamp();
  const whole = await page.screenshot({ type: "png", animations: "allow", caret: "initial", fullPage: false, timeout: 15_000 });
  await writeFile(path.join(directory, `${prefix}-viewport.png`), whole, { flag: "wx" });
  const canvas = await surface.screenshot({ type: "png", animations: "allow", caret: "initial", timeout: 15_000 });
  await writeFile(path.join(directory, `${prefix}-surface.png`), canvas, { flag: "wx" });
  const viewportPixels = inspectImage(whole, config.contract.imageChecks);
  requireCondition(viewportPixels.width === viewport.width && viewportPixels.height === viewport.height, "Screenshot pixel dimensions differ from viewport");
  const surfacePixels = inspectImage(canvas, config.contract.imageChecks);
  return {
    label, startedAt, finishedAt: timestamp(), viewport, surfaceBounds: box,
    viewportImage: { path: `${prefix}-viewport.png`, sha256: sha256(whole), pixels: viewportPixels },
    surfaceImage: { path: `${prefix}-surface.png`, sha256: sha256(canvas), pixels: surfacePixels },
  };
}

async function harnessDigest(): Promise<string> {
  const sources = (await readdir(path.join(harnessRoot, "src"))).filter((f) => f.endsWith(".ts")).sort();
  const files = ["package.json", "pnpm-lock.yaml", ...sources.map((f) => `src/${f}`),
    "fixtures/fixture.html", "fixtures/triangle.wgsl"];
  const hashes = await Promise.all(files.map(async (file) => [file, sha256(await readFile(path.join(harnessRoot, file)))]));
  return sha256(canonicalJson(hashes));
}

export async function runHarness(input: HarnessConfig, options: { runId?: string; signal?: AbortSignal; inspectionMs?: number } = {}) {
  const config = parseConfig(input);
  assertNativeConfiguration(config);
  const inspectionMs = options.inspectionMs ?? 0;
  requireCondition(Number.isInteger(inspectionMs) && inspectionMs >= 0 && inspectionMs <= 300_000,
    "Owner inspection hold must be 0..300000 ms");
  requireCondition(!inspectionMs || config.purpose === "tool-fixture", "Manual inspection is probe-only, never inside a candidate benchmark");
  requireCondition(await realpath(process.cwd()) === await realpath(harnessRoot), "Run the harness from tools/browser-harness (short local socket paths)");
  const runId = options.runId ?? `${timestamp().replace(/[:.]/g, "-").toLowerCase()}-${randomUUID().slice(0, 8)}`;
  requireCondition(/^[a-z0-9][a-z0-9_-]{0,100}$/.test(runId), "Run ID must be a simple lowercase identifier, not a path");
  const runs = path.join(harnessRoot, "runs");
  await mkdir(runs, { recursive: true });
  requireCondition(await realpath(runs) === runs, "Run directory must not be a symlink");
  const directory = path.join(runs, runId);
  await mkdir(directory, { mode: 0o700 });
  const runtime = path.join(harnessRoot, ".runtime", `b-${randomUUID().slice(0, 8)}`);
  await mkdir(runtime, { recursive: true, mode: 0o700 });
  const originalTmp = process.env.TMPDIR;
  process.env.TMPDIR = runtimeEnvironment(process.platform, runtime, process.env).TMPDIR;
  let context: BrowserContext | undefined;
  let browserPid: number | null = null;
  let browserExitVerified: boolean | null = null;
  const logs: LogEntry[] = [];
  const failures: string[] = [];
  const captures: Awaited<ReturnType<typeof saveCapture>>[] = [];
  let closed = false;
  const record = (kind: string, message: string, url?: string) => {
    if (logs.length < 10_000) logs.push({ at: timestamp(), kind, message, ...(url ? { url } : {}) });
    else if (!failures.includes("Log capacity exceeded")) failures.push("Log capacity exceeded");
  };
  const assertHealthy = () => {
    options.signal?.throwIfAborted();
    const errors = logs.filter((e) => ["console-error", "pageerror", "requestfailed", "http-error", "blocked-url", "crash", "popup"].includes(e.kind));
    requireCondition(errors.length === 0, `Browser/runtime/network failures: ${errors.slice(0, 3).map((e) => `${e.kind}: ${e.message}`).join("; ")}`);
  };
  const report: Record<string, unknown> = {
    schemaVersion: 1,
    runId,
    startedAt: timestamp(),
    status: "running",
    purpose: config.purpose,
    m1Acceptance: "not-evaluated",
    securityCertification: "not-performed",
    baselineApproved: false,
    evidenceLimits: config.purpose === "tool-fixture"
      ? "TOOL FIXTURE ONLY. Not a game renderer, source fidelity, representative workload, or performance acceptance."
      : "Unreviewed observations only. Does not establish gameplay, source fidelity, audio, both-browser coverage, representative hardware, or owner acceptance.",
    configuration: config,
    benchmarkContract: { id: config.contract.id, sha256: contractHash(config.contract), canonicalization: "recursive lexicographic JSON keys; UTF-8; no whitespace" },
    sourcePack: config.contract.sourcePack,
    display: { mode: process.env.CLUBSCAPE_DISPLAY_MODE ?? "unmanaged", display: process.platform === "linux" ? process.env.DISPLAY ?? null : null },
    input: { keyboard: true, mouse: true, mobile: false, touch: false },
    captureSettings: { headless: false, chromiumSandbox: true, locale: "en-US", timezoneId: "UTC", colorScheme: "light", animations: "allow", caret: "initial" },
    host: {
      ...(process.platform === "linux" ? { hostname: os.hostname() } : {}),
      platform: os.platform(), release: os.release(), arch: os.arch(),
      cpuModels: [...new Set(os.cpus().map((c) => c.model))], logicalCpus: os.cpus().length, totalMemoryBytes: os.totalmem(),
    },
    captures, failures,
  };
  try {
    await verifySourcePack(config);
    const nativeHost = await collectHostFacts();
    report.nativeHost = nativeHost;
    report.hostFingerprint = hostFingerprint(nativeHost);
    if (process.platform === "darwin") requireOwnerMac(nativeHost);
    requireCondition(!config.browser.hostFingerprint || config.browser.hostFingerprint === report.hostFingerprint,
      "Host model/chip/memory/OS/GPU inventory changed; recollect and repin before measurement");
    report.harness = { version: "0.1.0", sourceSha256: await harnessDigest(), node: process.version, playwright: createRequire(import.meta.url)("playwright-core/package.json").version };
    const executable = await executableMetadata(config);
    report.executable = executable;
    options.signal?.throwIfAborted();
    context = await chromium.launchPersistentContext(path.join(runtime, "profile"), {
      executablePath: executable.path,
      headless: false,
      chromiumSandbox: true,
      ignoreDefaultArgs: ["--enable-unsafe-swiftshader"],
      args: ["--enable-automation", ...graphicsProfiles[config.browser.graphicsProfile]],
      viewport: { width: config.contract.viewport.width, height: config.contract.viewport.height },
      deviceScaleFactor: 1,
      isMobile: false,
      hasTouch: false,
      locale: "en-US",
      timezoneId: "UTC",
      colorScheme: "light",
      reducedMotion: "no-preference",
      acceptDownloads: false,
      serviceWorkers: "block",
      timeout: 45_000,
      env: runtimeEnvironment(process.platform, runtime, process.env),
    });
    context.setDefaultTimeout(config.contract.measurement.readinessTimeoutMs);
    const browser = await browserMetadata(context, config);
    report.browser = browser;
    browserPid = browser.browserPid;
    validateBrowserBackend(browser.gpu, config);
    validateCandidateSandbox(config, browser.sandbox);
    report.securityEvidenceStatus = browser.sandbox.nativeAttestation;
    report.graphicsEvidence = {
      expectedProfile: config.browser.graphicsProfile,
      compositorBackend: browser.gpu.auxAttributes?.displayType ?? null,
      note: "CDP compositor/ANGLE metadata and the configured WebGPU device are separate observations; neither proves physical presentation",
    };
    await context.addInitScript(installGpuObserver);
    await context.route("**/*", async (route) => {
      const url = route.request().url();
      if (isAllowedUrl(url, config.allowedOrigins)) await route.continue();
      else { record("blocked-url", "Request blocked before network connection", url); await route.abort("blockedbyclient"); }
    });
    await context.routeWebSocket("**/*", (socket) => {
      if (isAllowedUrl(socket.url(), config.allowedOrigins, true)) socket.connectToServer();
      else { record("blocked-url", "WebSocket blocked before connection", socket.url()); socket.close({ code: 1008, reason: "Harness URL allowlist" }); }
    });
    const page = context.pages()[0] ?? await context.newPage();
    context.on("page", (popup) => { record("popup", "Unexpected additional page"); void popup.close(); });
    page.on("console", (event) => record(event.type() === "error" ? "console-error" : `console-${event.type()}`, event.text(), event.location().url));
    page.on("pageerror", (error) => record("pageerror", error.stack ?? error.message));
    page.on("crash", () => record("crash", "Renderer process crashed"));
    context.on("requestfailed", (request) => { if (!closed) record("requestfailed", request.failure()?.errorText ?? "Unknown request failure", request.url()); });
    context.on("response", (response) => record(response.status() >= 400 ? "http-error" : "response", `${response.status()} ${response.request().method()}`, response.url()));
    await page.goto(config.url, { waitUntil: "load", timeout: config.contract.measurement.readinessTimeoutMs });
    requireCondition(isAllowedUrl(page.url(), config.allowedOrigins), "Navigation escaped target allowlist");
    report.actualUrl = page.url();
    const fixtureMarker = await page.locator("html").getAttribute("data-evidence-kind");
    requireCondition(config.purpose === "tool-fixture" ? fixtureMarker === "tool-fixture" : fixtureMarker !== "tool-fixture",
      "Tool fixture marker cannot be absent, misclassified as candidate, or used as a source reference");
    for (const action of config.contract.setupActions) {
      if (action.type === "key-press") await page.keyboard.press(action.key);
      else if (action.type === "mouse-click") await page.mouse.click(action.x, action.y, { button: action.button });
      else if (action.type === "mouse-move") await page.mouse.move(action.x, action.y);
      else await page.mouse.wheel(action.deltaX, action.deltaY);
      record("desktop-input", JSON.stringify(action));
    }
    report.desktopCapabilities = await page.evaluate(() => ({
      finePointer: matchMedia("(pointer: fine)").matches,
      hover: matchMedia("(hover: hover)").matches,
      maxTouchPoints: navigator.maxTouchPoints,
    }));
    let initial: Sample | undefined;
    if (config.mode === "benchmark") {
      await page.waitForFunction(() => typeof window.__clubscapeBenchmarkV1?.read === "function",
        undefined, { timeout: config.contract.measurement.readinessTimeoutMs }).catch(() => {
        throw new Error("Missing or unready rendered-frame instrumentation");
      });
      report.contractBinding = await bounded(page.evaluate((binding) => {
        const api = window.__clubscapeBenchmarkV1!;
        if (!api.bindRun) return "preconfigured-by-client";
        const result: unknown = api.bindRun(binding);
        if (result !== null && typeof result === "object" && "then" in result) {
          throw new Error("Benchmark bindRun() must be synchronous and bind audit identity only");
        }
        return "renderer-bindRun-audit-identity-only";
      }, { contractId: config.contract.id, contractSha256: contractHash(config.contract) }), "Benchmark audit binding", 5000);
      await page.waitForFunction(() => {
        try { return window.__clubscapeBenchmarkV1?.read(null)?.ready === true; } catch { return false; }
      }, undefined, { timeout: config.contract.measurement.readinessTimeoutMs }).catch(() => {
        throw new Error("Missing or unready rendered-frame instrumentation");
      });
      const rawInitial = await samplePage(page, config, null);
      report.lastRendererSample = rawInitial;
      initial = validateSample(rawInitial, config);
      report.initialRenderer = initial;
    } else if (config.purpose !== "source-observation") {
      await page.waitForFunction((selector) => {
        return !!(window as any).__clubscapeHarnessObserverV1?.read(selector).device;
      }, config.contract.surfaceSelector);
      const gpu = await page.evaluate((selector) => (window as any).__clubscapeHarnessObserverV1.read(selector), config.contract.surfaceSelector);
      validateGpu(gpu, config);
      report.rendererGpu = gpu;
    }
    assertHealthy();
    captures.push(await saveCapture(page, config, directory, "primary-before", config.contract.viewport));
    if (config.mode === "benchmark") {
      await delay(config.contract.measurement.warmupMs, undefined, { signal: options.signal });
      const start = validateSample(await samplePage(page, config, null), config);
      let last = start;
      const samples: Sample[] = [start];
      const frames: RenderFrame[] = [];
      const budget = config.contract.measurement;
      while (last.observedAtMs - start.observedAtMs < budget.durationMs) {
        assertHealthy();
        await delay(Math.min(budget.pollMs, Math.max(1, budget.durationMs - (last.observedAtMs - start.observedAtMs))), undefined, { signal: options.signal });
        const raw = await samplePage(page, config, last.snapshot.renderedFrames);
        report.lastRendererSample = raw;
        last = validateSample(raw, config, last);
        frames.push(...last.snapshot.frames);
        samples.push(last);
      }
      const measurement = summarizeFrames(start, last, frames, config);
      report.measurement = measurement;
      report.queueCompletion = await bounded(page.evaluate(async (selector) => {
        return (window as any).__clubscapeHarnessObserverV1.drain(selector);
      }, config.contract.surfaceSelector), "Post-window GPU queue completion", 5000);
      await writeFile(path.join(directory, "rendered-frames.json"), JSON.stringify({
        clock: "performance.now", start, end: last,
        sampleObservations: samples.map(({ snapshot, ...sample }) => ({
          ...sample, renderedFrames: snapshot.renderedFrames, lastSubmittedAtMs: snapshot.lastSubmittedAtMs, lastCompletedAtMs: snapshot.lastCompletedAtMs,
        })),
        frames,
      }, null, 2), { flag: "wx" });
      if (measurement.budgetDecision === "fail") failures.push(...measurement.budgetFailures);
      assertHealthy();
      captures.push(await saveCapture(page, config, directory, "primary-after", config.contract.viewport));
    }
    for (const size of config.contract.resizeChecks) {
      const viewport = { ...size, deviceScaleFactor: 1 as const };
      await page.setViewportSize(size);
      if (config.mode === "benchmark") {
        await page.waitForFunction(({ width, height, selector }) => {
          const s = window.__clubscapeBenchmarkV1?.read(null);
          const canvas = document.querySelector(selector);
          const bounds = canvas?.getBoundingClientRect();
          return s?.ready && s.viewport.width === width && s.viewport.height === height
            && canvas instanceof HTMLCanvasElement && bounds
            && canvas.width === Math.round(bounds.width) && canvas.height === Math.round(bounds.height);
        }, { ...size, selector: config.contract.surfaceSelector });
        validateSample(await samplePage(page, config, null), config, undefined, viewport);
      } else {
        await delay(250, undefined, { signal: options.signal });
        if (config.purpose !== "source-observation") {
          const gpu = await page.evaluate((selector) => (window as any).__clubscapeHarnessObserverV1.read(selector), config.contract.surfaceSelector);
          validateGpu(gpu, config);
        }
      }
      captures.push(await saveCapture(page, config, directory, `resize-${size.width}x${size.height}`, viewport));
      assertHealthy();
    }
    await page.setViewportSize(config.contract.viewport);
    assertHealthy();
    if (inspectionMs) {
      const session = await context.browser()!.newBrowserCDPSession();
      try {
        const processes = await session.send("SystemInfo.getProcessInfo");
        const inspection = {
          purpose: "owner-native-inspection-only; outside measurement",
          startedAt: timestamp(), maximumDurationMs: inspectionMs,
          executable, sandbox: browser.sandbox,
          processes: processes.processInfo.map(({ id, type }) => ({ pid: id, type })),
          instructions: "Use these owned PIDs in Activity Monitor; record Kind/Sandbox if offered. Missing fields remain unknown. Do not disable SIP, Gatekeeper or Seatbelt.",
        };
        report.inspection = inspection;
        await writeFile(path.join(directory, "live-inspection.json"), JSON.stringify(inspection, null, 2), { flag: "wx" });
        console.log(`Owner inspection window: ${inspectionMs / 1000}s; ${directory}/live-inspection.json`);
        await delay(inspectionMs, undefined, { signal: options.signal });
      } finally { await session.detach(); }
    }
    report.status = failures.length ? "failed" : config.mode === "benchmark" ? "valid-measurement" : "valid-capture";
  } catch (error) {
    failures.push(error instanceof Error ? error.message : String(error));
    report.status = "failed";
  } finally {
    closed = true;
    if (context) {
      try { await bounded(context.close(), "Owned browser cleanup", 10_000); }
      catch (error) {
        failures.push(String(error));
        report.status = "failed";
        if (browserPid) {
          try { process.kill(browserPid, "SIGTERM"); } catch { /* Process may already have exited. */ }
        }
      }
    }
    if (browserPid) {
      const alive = () => {
        try { process.kill(browserPid!, 0); return true; } catch (error) {
          if ((error as NodeJS.ErrnoException).code === "ESRCH") return false;
          const message = `Cannot verify owned browser exit: ${String(error)}`;
          if (!failures.includes(message)) failures.push(message);
          return true;
        }
      };
      for (let attempt = 0; attempt < 40 && alive(); attempt++) {
        try {
          if (attempt === 10) process.kill(browserPid, "SIGTERM");
          if (attempt === 30) process.kill(browserPid, "SIGKILL");
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code !== "ESRCH") failures.push(`Owned browser cleanup: ${String(error)}`);
        }
        await delay(100);
      }
      browserExitVerified = !alive();
      if (!browserExitVerified) { failures.push("Owned browser process did not exit"); report.status = "failed"; }
    }
    await rm(runtime, { recursive: true, force: true });
    if (originalTmp === undefined) delete process.env.TMPDIR;
    else process.env.TMPDIR = originalTmp;
    report.finishedAt = timestamp();
    if (failures.length) report.status = "failed";
    report.cleanup = { browserCloseRequested: !!context, browserPid, browserExitVerified, runtimeRemoved: true };
    await writeFile(path.join(directory, "events.json"), JSON.stringify(logs, null, 2), { flag: "wx" });
    await writeFile(path.join(directory, "report.json"), JSON.stringify(report, null, 2), { flag: "wx" });
  }
  return { ok: report.status !== "failed", directory, report, failures };
}
