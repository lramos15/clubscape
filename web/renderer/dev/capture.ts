/**
 * Real-browser validation capture for the renderer fixture page.
 *
 * Launches the pinned windowed Chrome (sandbox on, hardware WebGPU/Vulkan profile from
 * docs/machines/sparky.md), renders each approved fixture scene through the adapter at 1920x1080
 * DPR 1, screenshots the visible canvas element, records GPU-completed frame statistics from the
 * page's RenderSnapshot protocol, runs resize/pick/actor checks and writes a report.
 *
 * Run under Xvfb from the repository root, e.g.
 *   xvfb-run --auto-servernum --server-args="-screen 0 2720x1600x24 -nolisten tcp" \
 *     node web/renderer/dev/capture.ts --out .local/render-assets/browser/run1
 */
import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright-core";
import type { Page } from "playwright-core";

const SCENES = ["lumbridge-castle-plaza", "lumbridge-river-bridge", "tutorial-starting-house", "tutorial-survival-coast", "lumbridge-windmill-route"];
const GRAPHICS_ARGS = ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=vulkan", "--enable-gpu", "--ignore-gpu-blocklist", "--ozone-platform=x11"];

function argValue(name: string, fallback: string): string {
  const index = process.argv.indexOf(name);
  return index >= 0 && process.argv[index + 1] ? process.argv[index + 1]! : fallback;
}

const out = path.resolve(argValue("--out", ".local/render-assets/browser/latest"));
const port = Number(argValue("--port", "4173"));
const executable = process.env.CLUBSCAPE_CHROME ?? path.join(process.env.HOME ?? "", ".cache/ms-playwright/chromium-1243/chrome-linux-arm64/chrome");
const measureMs = Number(argValue("--measure-ms", "5000"));

interface DevProbe { ready: boolean; error: string | null; sceneId: string }

async function waitReady(page: Page, timeoutMs = 180_000): Promise<void> {
  const started = Date.now();
  for (;;) {
    const probe = await page.evaluate((): DevProbe | null => {
      const dev = window.__clubscapeDev;
      return dev ? { ready: dev.ready, error: dev.error, sceneId: dev.sceneId } : null;
    });
    if (probe?.error) throw new Error(`fixture page error: ${probe.error}`);
    if (probe?.ready) return;
    if (Date.now() - started > timeoutMs) throw new Error("fixture page did not become ready");
    await page.waitForTimeout(250);
  }
}

async function waitFrames(page: Page, count: number, timeoutMs = 60_000): Promise<void> {
  const started = Date.now();
  for (;;) {
    const rendered = await page.evaluate(() => window.__clubscapeDev?.handle?.diagnostics().renderedFrames ?? 0);
    if (rendered >= count) return;
    if (Date.now() - started > timeoutMs) throw new Error(`only ${rendered} GPU-completed frames after ${timeoutMs} ms`);
    await page.waitForTimeout(50);
  }
}

async function snapshot(page: Page, afterFrame: number | null) {
  return page.evaluate((after) => window.__clubscapeBenchmarkV1!.read(after), afterFrame) as Promise<any>;
}

function percentile(values: number[], p: number): number | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(p * sorted.length))] ?? null;
}

async function main(): Promise<void> {
  await mkdir(out, { recursive: true });
  const server = spawn(process.execPath, [path.resolve("web/renderer/dev/serve.ts"), String(port)], { stdio: ["ignore", "pipe", "inherit"] });
  await new Promise<void>((resolve, reject) => {
    server.stdout.once("data", () => resolve());
    server.once("exit", (code) => reject(new Error(`server exited ${code}`)));
  });
  const runtime = path.join(out, "profile");
  const report: Record<string, unknown> = { startedAt: new Date().toISOString(), executable, graphicsArgs: GRAPHICS_ARGS, scenes: {} };
  const context = await chromium.launchPersistentContext(runtime, {
    executablePath: executable,
    headless: false,
    chromiumSandbox: true,
    ignoreDefaultArgs: ["--enable-unsafe-swiftshader"],
    args: ["--enable-automation", ...GRAPHICS_ARGS],
    viewport: { width: 1920, height: 1080 },
    deviceScaleFactor: 1,
    locale: "en-US",
    timezoneId: "UTC",
    colorScheme: "light",
    serviceWorkers: "block",
    timeout: 60_000,
  });
  try {
    const browser = context.browser()!;
    const cdp = await browser.newBrowserCDPSession();
    const version = await cdp.send("Browser.getVersion");
    const system = await cdp.send("SystemInfo.getInfo");
    const commandLine = await cdp.send("Browser.getBrowserCommandLine");
    report.browser = { version, gpu: { auxAttributes: system.gpu.auxAttributes, featureStatus: system.gpu.featureStatus, devices: system.gpu.devices }, commandLine: commandLine.arguments };
    await cdp.detach();
    const featureStatus = (system.gpu.featureStatus ?? {}) as Record<string, string>;
    if (featureStatus.webgpu !== "enabled") throw new Error(`hardware WebGPU not enabled in this browser: ${JSON.stringify(featureStatus)}`);

    const page = await context.newPage();
    page.on("console", (message) => { if (message.type() === "error" || message.type() === "warning") console.log(`[page:${message.type()}] ${message.text()}`); });
    page.on("pageerror", (error) => console.log(`[pageerror] ${error.message}`));
    const scenes = report.scenes as Record<string, unknown>;

    for (const scene of SCENES) {
      await page.setViewportSize({ width: 1920, height: 1080 });
      await page.goto(`http://127.0.0.1:${port}/?scene=${scene}&w=1920&h=1080`);
      await waitReady(page);
      await waitFrames(page, 3);
      const first = await snapshot(page, null);
      const canvas = page.locator("canvas[data-clubscape-surface]");
      const box = await canvas.boundingBox();
      await canvas.screenshot({ path: path.join(out, `${scene}.png`), omitBackground: false });
      const before = await snapshot(page, null);
      await page.waitForTimeout(measureMs);
      const after = await snapshot(page, before.renderedFrames);
      const frames: Array<{ sequence: number; submittedAtMs: number; completedAtMs: number; cpuEncodeMs?: number; gpuDurationMs?: number; primitives: number; drawCalls: number }> = after.frames;
      const latency = frames.map((f) => f.completedAtMs - f.submittedAtMs);
      const gaps = frames.slice(1).map((f, i) => f.completedAtMs - frames[i]!.completedAtMs);
      scenes[scene] = {
        screenshot: `${scene}.png`,
        canvasBox: box,
        viewport: first.viewport,
        adapter: await page.evaluate(() => window.__clubscapeDev.handle!.diagnostics().adapter),
        timestampsSupported: await page.evaluate(() => window.__clubscapeDev.handle!.diagnostics().timestampsSupported),
        assets: first.assets,
        primitives: frames[0]?.primitives ?? null,
        drawCalls: frames[0]?.drawCalls ?? null,
        measured: {
          windowMs: measureMs,
          gpuCompletedFrames: frames.length,
          fps: frames.length / (measureMs / 1000),
          cpuEncodeMs: { p50: percentile(frames.map((f) => f.cpuEncodeMs ?? 0), 0.5), p95: percentile(frames.map((f) => f.cpuEncodeMs ?? 0), 0.95) },
          gpuDurationMs: { p50: percentile(frames.flatMap((f) => f.gpuDurationMs === undefined ? [] : [f.gpuDurationMs]), 0.5), p95: percentile(frames.flatMap((f) => f.gpuDurationMs === undefined ? [] : [f.gpuDurationMs]), 0.95), known: frames.filter((f) => f.gpuDurationMs !== undefined).length },
          submitToCompleteMs: { p50: percentile(latency, 0.5), p95: percentile(latency, 0.95), max: latency.length ? Math.max(...latency) : null },
          completionGapMs: { p50: percentile(gaps, 0.5), p95: percentile(gaps, 0.95), max: gaps.length ? Math.max(...gaps) : null },
        },
      };
      console.log(`${scene}: ${frames.length} GPU-completed frames in ${measureMs} ms, prims=${frames[0]?.primitives}, gpu p50=${(scenes[scene] as any).measured.gpuDurationMs.p50}`);
    }

    // Region mode: the world assembled from blocks, following a player across a map-square edge.
    const regionResults: unknown[] = [];
    if (argValue("--regions", "1") === "1") {
      await page.setViewportSize({ width: 1920, height: 1080 });
      await page.goto(`http://127.0.0.1:${port}/?scene=region.osrs.12850&px=3222&py=3218&w=1920&h=1080`);
      await waitReady(page, 300_000);
      await waitFrames(page, 3);
      const canvas = page.locator("canvas[data-clubscape-surface]");
      const shot = async (name: string) => {
        await canvas.screenshot({ path: path.join(out, name) });
        return page.evaluate(() => {
          const d = window.__clubscapeDev.handle!.diagnostics();
          return { sceneId: d.sceneId, sceneBase: d.sceneBase, loadedSquares: d.loadedSquares, lastFrame: d.lastFrame };
        });
      };
      regionResults.push({ step: "lumbridge-castle", tile: [3222, 3218], ...(await shot("region-lumbridge-castle-3222-3218.png")) });
      // Frozen representative workload: geared player, animated NPCs, fire and ground items in the
      // streamed Lumbridge scene; every counted frame is a GPU-completed record.
      const workloadMs = Number(argValue("--workload-ms", "0"));
      if (workloadMs > 0) {
        const applied = await page.evaluate(() => window.__clubscapeDev.applyScenario!("workload"));
        await waitFrames(page, (await snapshot(page, null)).renderedFrames + 5);
        await canvas.screenshot({ path: path.join(out, "region-workload-lumbridge-castle.png") });
        const before = await snapshot(page, null);
        const started = Date.now();
        await page.waitForTimeout(workloadMs);
        const after = await snapshot(page, before.renderedFrames);
        const elapsedMs = Date.now() - started;
        const frames: Array<{ sequence: number; submittedAtMs: number; completedAtMs: number; cpuEncodeMs?: number; gpuDurationMs?: number; primitives: number }> = after.frames;
        const latency = frames.map((f) => f.completedAtMs - f.submittedAtMs);
        const gaps = frames.slice(1).map((f, i) => f.completedAtMs - frames[i]!.completedAtMs);
        const counted = after.renderedFrames - before.renderedFrames;
        const workload = {
          screenshot: "region-workload-lumbridge-castle.png", applied, windowMs: workloadMs, elapsedMs,
          gpuCompletedFrames: counted, recordedFrames: frames.length, fps: counted / (elapsedMs / 1000),
          primitives: { min: Math.min(...frames.map((f) => f.primitives)), max: Math.max(...frames.map((f) => f.primitives)) },
          cpuEncodeMs: { p50: percentile(frames.map((f) => f.cpuEncodeMs ?? 0), 0.5), p95: percentile(frames.map((f) => f.cpuEncodeMs ?? 0), 0.95), max: Math.max(...frames.map((f) => f.cpuEncodeMs ?? 0)) },
          gpuDurationMs: { p50: percentile(frames.flatMap((f) => f.gpuDurationMs === undefined ? [] : [f.gpuDurationMs]), 0.5), p95: percentile(frames.flatMap((f) => f.gpuDurationMs === undefined ? [] : [f.gpuDurationMs]), 0.95), known: frames.filter((f) => f.gpuDurationMs !== undefined).length },
          submitToCompleteMs: { p50: percentile(latency, 0.5), p95: percentile(latency, 0.95), max: latency.length ? Math.max(...latency) : null },
          completionGapMs: { p50: percentile(gaps, 0.5), p95: percentile(gaps, 0.95), max: gaps.length ? Math.max(...gaps) : null, over33ms: gaps.filter((g) => g > 33.4).length },
          note: "Xvfb headful Chrome 153 on the Sparky GB10; frames are queue-completion records, not requestAnimationFrame counts. Not an owner/Mac/Edge acceptance measurement.",
        };
        regionResults.push({ step: "workload", tile: [3222, 3218], ...workload });
        console.log(`workload ${workloadMs} ms: ${counted} GPU-completed frames (${workload.fps.toFixed(2)} fps), prims ${workload.primitives.min}..${workload.primitives.max}, gpu p95 ${workload.gpuDurationMs.p95}, gap p95 ${workload.completionGapMs.p95} max ${workload.completionGapMs.max}, gaps>33ms ${workload.completionGapMs.over33ms}`);
        await page.evaluate(() => window.__clubscapeDev.walkTo!(3222, 3218));
      }
      // Walk north-west in 4-tile steps toward Draynor's square edge; the scene must recenter.
      const route: Array<[number, number]> = [[3210, 3230], [3200, 3240], [3190, 3250], [3180, 3260], [3170, 3270], [3160, 3280]];
      for (const [x, y] of route) {
        await page.evaluate(([px, py]) => window.__clubscapeDev.walkTo!(px!, py!), [x, y] as [number, number]);
        await page.waitForTimeout(400);
        await waitFrames(page, (await snapshot(page, null)).renderedFrames + 2);
      }
      regionResults.push({ step: "after-recenter", tile: [3160, 3280], ...(await shot("region-after-recenter-3160-3280.png")) });
      await page.evaluate(() => window.__clubscapeDev.walkTo!(3096, 3105));
      await page.waitForTimeout(1500);
      await waitFrames(page, (await snapshot(page, null)).renderedFrames + 3);
      regionResults.push({ step: "tutorial-island", tile: [3096, 3105], ...(await shot("region-tutorial-island-3096-3105.png")) });
      const picks = await page.evaluate(() => {
        const handle = window.__clubscapeDev.handle!;
        return [[960, 540], [700, 600], [1200, 700]].map(([x, y]) => ({ x, y, pick: handle.pick(x!, y!) }));
      });
      regionResults.push({ step: "picks", picks });
      console.log("region mode", JSON.stringify(regionResults.map((r: any) => ({ step: r.step, base: r.sceneBase, squares: r.loadedSquares?.length, prims: r.lastFrame?.primitives, picks: r.picks }))));
    }
    report.regions = regionResults;

    // Approved model/animation captures replayed through the legacy draw path in the browser.
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto(`http://127.0.0.1:${port}/?mode=model&w=1920&h=1080`);
    await waitReady(page);
    const modelCases: Array<{ name: string; request: Record<string, unknown> }> = [];
    for (const yaw of [0, 256, 512, 1024]) {
      modelCases.push({ name: `tree-1277-yaw-${yaw}`, request: { model: "models/object-1277-model-1570-lit.bin", yaw, cameraY: 250, cameraZ: 750 } });
    }
    for (const [seq, frames] of [[6181, 16], [6180, 16]] as const) {
      for (let frame = 0; frame < frames; frame += 1) {
        modelCases.push({ name: `npc-3028-sequence-${seq}-frame-${frame}`, request: { npc: { id: 3028, sequence: seq, frame }, yaw: 256, cameraY: 240, cameraZ: 650 } });
      }
    }
    for (const [seq, frames] of [[5668, 14], [5666, 8]] as const) {
      for (let frame = 0; frame < frames; frame += 1) {
        modelCases.push({ name: `npc-2063-sequence-${seq}-frame-${frame}`, request: { npc: { id: 2063, sequence: seq, frame }, yaw: 256, cameraY: 160, cameraZ: 400 } });
      }
    }
    const modelDir = path.join(out, "models");
    await mkdir(modelDir, { recursive: true });
    const modelResults: unknown[] = [];
    for (const c of modelCases) {
      const frame = await page.evaluate((request) => window.__clubscapeDev.handle!.frameModelFixture(request as any), c.request);
      await page.locator("canvas[data-clubscape-surface]").screenshot({ path: path.join(modelDir, `${c.name}.png`) });
      modelResults.push({ name: c.name, request: c.request, frame });
    }
    report.models = modelResults;
    console.log(`model fixtures captured: ${modelResults.length}`);

    // Live-layer and player-action scenarios on the starting-house fixture (developer WorldViews
    // with real source ids): gear, activity motions, ground items + fire, door state, roof mode
    // and the model-only interface preview readback.
    const scenarioNames = ["pinned-gear-idle", "gear-idle", "gear-fighting", "woodcutting", "mining", "fishing", "firemaking", "cooking", "walking", "ranged", "casting", "death", "ground-items-fire", "door-open", "roof-player", "preview"];
    const scenarioDir = path.join(out, "scenarios");
    await mkdir(scenarioDir, { recursive: true });
    const scenarioResults: unknown[] = [];
    if (argValue("--scenarios", "1") === "1") {
      await page.setViewportSize({ width: 1920, height: 1080 });
      await page.goto(`http://127.0.0.1:${port}/?scene=tutorial-starting-house&w=1920&h=1080`);
      await waitReady(page);
      await waitFrames(page, 3);
      const canvas = page.locator("canvas[data-clubscape-surface]");
      for (const name of scenarioNames) {
        const applied = await page.evaluate((n) => window.__clubscapeDev.applyScenario!(n), name);
        const startFrames = (await snapshot(page, null)).renderedFrames;
        await waitFrames(page, startFrames + 3);
        await canvas.screenshot({ path: path.join(scenarioDir, `${name}-t0.png`) });
        await page.waitForTimeout(600);
        await canvas.screenshot({ path: path.join(scenarioDir, `${name}-t1.png`) });
        if (name === "preview") {
          await page.locator("canvas[data-clubscape-preview]").screenshot({ path: path.join(scenarioDir, "preview-surface.png"), omitBackground: true });
        }
        const last = await page.evaluate(() => window.__clubscapeDev.handle!.diagnostics().lastFrame);
        const picks = await page.evaluate(() => {
          const handle = window.__clubscapeDev.handle!;
          const points: Array<[number, number]> = [[960, 540], [1030, 560], [880, 520], [1100, 470]];
          return points.map(([x, y]) => ({ x, y, pick: handle.pick(x, y) }));
        });
        scenarioResults.push({ name, applied, lastFrame: last, picks });
        console.log(`scenario ${name}: prims=${last?.primitives} ${JSON.stringify(applied).slice(0, 200)}`);
      }
      await page.evaluate(() => window.__clubscapeDev.applyScenario!("pinned-gear-idle"));
    }
    report.scenarios = scenarioResults;

    // Resize range checks on one scene.
    const resizes: unknown[] = [];
    for (const [w, h] of [[1024, 768], [1280, 720], [2560, 1440]] as const) {
      await page.setViewportSize({ width: w, height: h });
      await page.goto(`http://127.0.0.1:${port}/?scene=lumbridge-castle-plaza&w=${w}&h=${h}`);
      await waitReady(page);
      await waitFrames(page, 3);
      const canvas = page.locator("canvas[data-clubscape-surface]");
      const file = `lumbridge-castle-plaza-${w}x${h}.png`;
      await canvas.screenshot({ path: path.join(out, file) });
      resizes.push({ width: w, height: h, screenshot: file, box: await canvas.boundingBox(), primitives: (await snapshot(page, null)).renderedFrames });
    }
    report.resizes = resizes;

    // Live resize through the handle (same page) from 1920x1080 to 1280x720 and back.
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto(`http://127.0.0.1:${port}/?scene=tutorial-starting-house&w=1920&h=1080&actors=1`);
    await waitReady(page);
    await waitFrames(page, 3);
    await page.evaluate(() => window.__clubscapeDev.resizeTo(1280, 720));
    await waitFrames(page, 8);
    await page.locator("canvas[data-clubscape-surface]").screenshot({ path: path.join(out, "tutorial-starting-house-actors-live-resize-1280x720.png") });
    await page.evaluate(() => window.__clubscapeDev.resizeTo(1920, 1080));
    await waitFrames(page, 12);
    await page.locator("canvas[data-clubscape-surface]").screenshot({ path: path.join(out, "tutorial-starting-house-actors-t0.png") });
    await page.waitForTimeout(400);
    await page.locator("canvas[data-clubscape-surface]").screenshot({ path: path.join(out, "tutorial-starting-house-actors-t1.png") });
    const picks = await page.evaluate(() => {
      const handle = window.__clubscapeDev.handle!;
      const points: Array<[number, number]> = [[960, 540], [1280, 800], [1320, 880], [1300, 900], [100, 100], [1900, 1070]];
      return points.map(([x, y]) => ({ x, y, pick: handle.pick(x, y) }));
    });
    report.picks = picks;
    report.actorsSkipped = await page.evaluate(() => window.__clubscapeDev.handle!.diagnostics().lastFrame);
    console.log("picks", JSON.stringify(picks));
    await page.close();
  } finally {
    await context.close();
    server.kill("SIGTERM");
  }
  report.finishedAt = new Date().toISOString();
  await writeFile(path.join(out, "report.json"), JSON.stringify(report, null, 2));
  console.log(`report ${path.join(out, "report.json")}`);
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
