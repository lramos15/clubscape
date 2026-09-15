import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import type { BrowserContext, Page } from "playwright-core";
import { assertSourceRunPin, captureSourceRunPin } from "../../../tools/web-build/run-pins.ts";
import type { SourceRunPin } from "../../../tools/web-build/run-pins.ts";
import type { PublicWorld } from "../public-state.ts";
import { sourceAudioDefaults } from "../../audio/index.ts";
import type { RenderSnapshot } from "../../../tools/browser-harness/src/protocol.ts";
import type { PreviewObservation } from "../preview.ts";
import { presentationOptions } from "../presentation.ts";

const root = resolve(fileURLToPath(new URL("../../../", import.meta.url)));
type SecretWindow = Window & { __sourceUiCredentials?: { name: string; password: string } };
type FaultWindow = Window & { __clubscapeFailureDevice?: GPUDevice };
type ObservedSnapshot = RenderSnapshot & { diagnostics: {
  loadedSquares: number[] | null; scenePlacement: { baseX: number; baseY: number; sizeTiles: number; blocks: boolean } | null;
  nativeScenePlacement: { baseX: number; baseY: number; sizeTiles: number; blocks: boolean } | null;
  modelPreview: PreviewObservation | null; playerAnimationAvailable: boolean | null;
} };

function distribution(values: number[]) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const at = (quantile: number) => sorted[Math.max(0, Math.ceil(sorted.length * quantile) - 1)]!;
  return { count: sorted.length, min: sorted[0], p50: at(0.5), p95: at(0.95), p99: at(0.99), max: sorted.at(-1) };
}

async function fields(page: Page, confirmation: boolean, wrongPassword = false): Promise<void> {
  await page.locator('input[data-ui-input="name"]').waitFor({ state: "attached" });
  await page.locator('input[data-ui-input="password"]').waitFor({ state: "attached" });
  if (confirmation) await page.locator('input[data-ui-input="confirmation"]').waitFor({ state: "attached" });
  await page.evaluate(({ confirmation, wrongPassword }) => {
    const scope = window as SecretWindow;
    if (!scope.__sourceUiCredentials) {
      const random = () => Array.from(crypto.getRandomValues(new Uint8Array(18)), (byte) => byte.toString(16).padStart(2, "0")).join("");
      scope.__sourceUiCredentials = { name: `ui_${random().slice(0, 14)}`, password: random() + random() };
    }
    const secret = scope.__sourceUiCredentials;
    for (const [id, value] of [["name", secret.name], ["password", wrongPassword ? "incorrect source UI test password" : secret.password],
      ...(confirmation ? [["confirmation", secret.password]] : [])]) {
      const element = document.querySelector<HTMLInputElement>(`input[data-ui-input="${id}"]`);
      if (!element) throw new Error("The actual source UI credential input is unavailable.");
      element.value = value!;
      element.dispatchEvent(new Event("input", { bubbles: true }));
    }
  }, { confirmation, wrongPassword });
}

function publicProgress(world: PublicWorld) {
  return {
    actorId: world.player.id, tutorialStage: world.player.tutorialStage,
    appearance: world.player.appearance, appearanceConfirmed: world.player.appearanceConfirmed,
    inventory: world.player.inventory, equipment: world.player.equipment,
    skills: world.player.skills, questPoints: world.player.questPoints, quests: world.player.quests,
  };
}

async function dismissNotices(page: Page): Promise<void> {
  for (let index = 0; index < 8; index++) {
    const notice = page.locator('[data-ui-control="notice-close"]');
    if (await notice.count() === 0) return;
    // Source notices can disappear when their asynchronous render completes.
    // Use the actual UI's stable keyboard cancellation path, not a stale button handle.
    await page.locator("#overlay").focus();
    await page.keyboard.press("Escape");
    await page.waitForTimeout(50);
  }
}

async function stopChild(child: ChildProcess): Promise<void> {
  if (child.exitCode !== null || child.signalCode !== null) return;
  child.kill("SIGTERM");
  await new Promise<void>((resolvePromise, reject) => {
    const timer = setTimeout(() => { child.kill("SIGKILL"); reject(new Error("Owned restart server did not stop gracefully.")); }, 20_000);
    child.once("exit", (code) => { clearTimeout(timer); code === 0 ? resolvePromise() : reject(new Error("Owned source server exited unsuccessfully.")); });
  });
}

async function restartSource(origin: string, pin: SourceRunPin, gameRoot: string): Promise<ChildProcess> {
  const pid = Number(process.env.CLUBSCAPE_SOURCE_SERVER_PID);
  const binary = process.env.CLUBSCAPE_SOURCE_SERVER_BINARY;
  const database = process.env.CLUBSCAPE_SOURCE_DATABASE_URL;
  assert(Number.isSafeInteger(pid) && pid > 1 && binary && database);
  await assertSourceRunPin(gameRoot, pin);
  process.kill(pid, "SIGTERM");
  const deadline = performance.now() + 20_000;
  while (performance.now() < deadline) {
    try { await fetch(`${origin}/healthz`, { signal: AbortSignal.timeout(500), redirect: "error" }); }
    catch { break; }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
  }
  // Real downtime long enough for the normal poll to observe transport loss.
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 800));
  const child = spawn(binary, [], {
    cwd: root, stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, DATABASE_URL: database, CLUBSCAPE_GAME_ROOT: gameRoot,
      CLUBSCAPE_WEB_ROOT: resolve(root, "web/dist"), CLUBSCAPE_BIND: new URL(origin).host },
  });
  try {
    await new Promise<void>((resolvePromise, reject) => {
      let pending = "";
      const timer = setTimeout(() => reject(new Error("Owned pinned source server restart timed out.")), 35_000);
      child.stdout!.on("data", (chunk: Buffer) => {
        pending += chunk.toString("utf8");
        if (pending.length > 64 * 1024) { clearTimeout(timer); reject(new Error("Unexpected source startup log volume.")); return; }
        for (;;) {
          const newline = pending.indexOf("\n");
          if (newline < 0) break;
          const line = pending.slice(0, newline); pending = pending.slice(newline + 1);
          const fields = (JSON.parse(line) as { fields?: { event?: string; address?: string } }).fields;
          if (fields?.event === "listening") {
            clearTimeout(timer);
            if (`http://${fields.address}` !== origin) reject(new Error("Source restart changed its pinned origin."));
            else resolvePromise();
          }
        }
      });
      child.stderr!.on("data", () => {});
      child.once("error", () => { clearTimeout(timer); reject(new Error("Owned source server could not restart.")); });
      child.once("exit", () => { clearTimeout(timer); reject(new Error("Owned source server exited before restart readiness.")); });
    });
    await assertSourceRunPin(gameRoot, pin);
    const health = await fetch(`${origin}/healthz`, { signal: AbortSignal.timeout(5000), redirect: "error" });
    assert.equal(health.status, 200);
    return child;
  } catch (error) { await stopChild(child); throw error; }
}

export async function sourceBrowserCheck(): Promise<void> {
  const origin = process.env.CLUBSCAPE_BROWSER_ORIGIN;
  const executable = process.env.CLUBSCAPE_BROWSER_EXECUTABLE;
  const gameRoot = process.env.CLUBSCAPE_SOURCE_GAME_ROOT;
  const earlyScene = process.env.CLUBSCAPE_EARLY_SCENE ?? null;
  const recordedCamera = process.env.CLUBSCAPE_RECORDED_CAMERA ?? null;
  const query = earlyScene ? `?presentation_scene=${earlyScene}` : recordedCamera ? `?presentation_camera=${recordedCamera}` : "";
  assert.deepEqual(presentationOptions(query), { earlyScene, recordedCamera });
  assert(origin && /^http:\/\/127\.0\.0\.1:\d+$/.test(origin) && executable && gameRoot && process.env.DISPLAY);
  const evidence = resolve(root, process.env.CLUBSCAPE_BROWSER_EVIDENCE ?? ".local/evidence/source-ui");
  assert(evidence.startsWith(root + sep));
  await mkdir(evidence, { recursive: true });
  const pin = await captureSourceRunPin(gameRoot);
  const runtime = resolve(root, ".local/c");
  await mkdir(runtime, { mode: 0o700 });
  const env: Record<string, string> = {};
  for (const key of ["PATH", "HOME", "DISPLAY", "XAUTHORITY", "LD_LIBRARY_PATH", "LANG"]) if (process.env[key]) env[key] = process.env[key]!;
  Object.assign(env, { TMPDIR: runtime, TMP: runtime, TEMP: runtime });
  let context: BrowserContext | undefined;
  let page: Page | undefined;
  let restarted: ChildProcess | null = null;
  const checks: string[] = [];
  try {
    context = await chromium.launchPersistentContext(resolve(evidence, "profile"), {
      executablePath: executable, chromiumSandbox: true, headless: false, env,
      ignoreDefaultArgs: ["--enable-unsafe-swiftshader"],
      args: ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=vulkan",
        "--enable-gpu", "--ignore-gpu-blocklist", "--ozone-platform=x11", "--enable-automation"],
      viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1,
    });
    const cdp = await context.browser()!.newBrowserCDPSession();
    const version = await cdp.send("Browser.getVersion");
    const system = await cdp.send("SystemInfo.getInfo");
    const command = await cdp.send("Browser.getBrowserCommandLine");
    assert(!command.arguments.some((argument) => /^(--no-sandbox|--disable-.*sandbox|--disable-web-security)(=|$)/.test(argument)));
    const sandbox = await context.newPage();
    await sandbox.goto("chrome://sandbox");
    const isolation = await sandbox.locator("body").innerText();
    assert.match(isolation, /Layer 1 Sandbox\s+Namespace/i);
    assert.match(isolation, /Seccomp-BPF\s+sandbox\s+Yes/i);
    await sandbox.close();
    checks.push("actual headful Chrome namespace/seccomp sandbox");
    page = await context.newPage();
    await page.addInitScript(() => {
      const configure = GPUCanvasContext.prototype.configure;
      GPUCanvasContext.prototype.configure = function (configuration) {
        configure.call(this, configuration);
        if (this.canvas instanceof HTMLCanvasElement && this.canvas.id === "world") {
          (window as FaultWindow).__clubscapeFailureDevice = configuration.device;
        }
      };
    });
    const unexpected = new Set<string>();
    const renderRequests = new Set<string>();
    const failedAssets = new Set<string>();
    let unhandledPageErrors = 0;
    page.on("pageerror", () => { unhandledPageErrors++; });
    page.on("response", (response) => {
      const path = new URL(response.url()).pathname;
      if (response.status() >= 400 && (path.startsWith("/assets/") || path.startsWith("/content/"))) failedAssets.add(path);
    });
    page.on("request", (request) => {
      const url = new URL(request.url());
      if (url.protocol === "data:" || url.protocol === "blob:") return;
      const permittedScene = query !== "" && url.pathname === "/" && url.search === query;
      if (url.origin !== origin || (url.search && !permittedScene) || url.hash || url.username || url.password) unexpected.add("noncanonical browser request");
      if (url.pathname.startsWith("/assets/compiled/render/")) renderRequests.add(url.pathname);
    });
    await page.goto(`${origin}${query}`, { waitUntil: "networkidle" });
    await page.waitForFunction(() => window.__clubscapeClientStateV1?.read().phase === "title", undefined, { timeout: 45_000 });
    await page.locator("#overlay").focus();
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "New account", exact: true }).waitFor();
    const title = await page.evaluate(() => {
      const canvas = document.querySelector<HTMLCanvasElement>("#overlay")!;
      const data = canvas.getContext("2d")!.getImageData(0, 0, canvas.width, canvas.height).data;
      let coloured = 0;
      for (let index = 0; index < data.length; index += 4) if (data[index + 3]! > 0 && (data[index]! > 12 || data[index + 1]! > 12 || data[index + 2]! > 12)) coloured++;
      return { width: canvas.width, height: canvas.height, coloured };
    });
    assert.deepEqual([title.width, title.height], [1920, 1080]);
    assert(title.coloured > 10_000, "Actual UI canvas is blank.");
    await page.screenshot({ path: resolve(evidence, "actual-source-title.png") });
    checks.push("real compiled source UI title, nonblank pixels and logical viewport");
    for (const { width, height } of [{ width: 1024, height: 768 }, { width: 2560, height: 1440 }, { width: 1920, height: 1080 }]) {
      await page.setViewportSize({ width, height });
      await page.waitForFunction(({ width, height }) => {
        const overlay = document.querySelector<HTMLCanvasElement>("#overlay")!;
        const world = document.querySelector<HTMLCanvasElement>("#world")!;
        return overlay.width === width && overlay.height === height
          && world.width === width * devicePixelRatio && world.height === height * devicePixelRatio;
      }, { width, height });
      await page.getByRole("button", { name: "New account", exact: true }).waitFor();
    }
    checks.push("actual logical UI/backing-pixel separation at approved minimum/maximum/primary resize sizes");
    const audioPositions = await page.evaluate(() => window.__clubscapeClientStateV1!.audioControls()?.channels);
    await page.getByRole("button", { name: "Enable sound", exact: true }).click();
    await page.waitForFunction(() => window.__clubscapeClientStateV1!.read().soundEnabled);
    await page.getByRole("button", { name: "Mute sound", exact: true }).click();
    await page.waitForFunction(() => !window.__clubscapeClientStateV1!.read().soundEnabled);
    const afterMute = await page.evaluate(() => window.__clubscapeClientStateV1!.audioControls()?.channels);
    assert.deepEqual(afterMute, audioPositions);
    checks.push("actual source title controls bind to the real audio graph, unlock on gesture and mute without resetting source positions");
    await page.getByRole("button", { name: "New account", exact: true }).click();
    await page.getByRole("button", { name: "Create account", exact: true }).waitFor();
    await fields(page, true);
    await page.getByRole("button", { name: "Create account", exact: true }).click();
    await page.waitForFunction(() => window.__clubscapeClientStateV1?.read().phase === "login");
    checks.push("actual UI registration through binary WASM/account RPC");
    await fields(page, false, true);
    await page.getByRole("button", { name: "Login", exact: true }).click();
    await page.waitForFunction(() => {
      const state = window.__clubscapeClientStateV1?.read();
      return state?.phase === "login" && typeof state.error?.errorId === "string" && state.world === null;
    });
    checks.push("real UI wrong-password rejection with server error ID and no world");
    await page.locator("#overlay").focus();
    await page.keyboard.press("Escape");
    await page.waitForTimeout(50);
    await page.keyboard.press("Escape");
    await fields(page, false);
    await page.getByRole("button", { name: "Login", exact: true }).click();
    await page.waitForFunction(() => window.__clubscapeClientStateV1?.read().phase === "character");
    assert.equal(await page.evaluate(() => window.__clubscapeClientStateV1!.read().world), null);
    checks.push("real UI login without seeded character/world");
    await page.waitForFunction(() => {
      const state = window.__clubscapeBenchmarkV1!.read(null) as ObservedSnapshot;
      return state.diagnostics.modelPreview?.state === "unavailable";
    }, undefined, { timeout: 30_000 });
    const preview = await page.evaluate(() => (window.__clubscapeBenchmarkV1!.read(null) as ObservedSnapshot).diagnostics.modelPreview);
    assert.deepEqual(preview?.nativeSize, { width: 480, height: 315 });
    assert.equal(preview?.purpose, "appearance");
    assert.equal(preview?.sourceWidget, 44499017);
    assert.match(preview?.problem ?? "", /base\/equipment metadata is unavailable/);
    assert.equal(preview?.publishedImages, 0);
    assert.equal(await page.evaluate(() => window.__clubscapeBenchmarkV1!.read(null).renderedFrames), 0);
    await page.screenshot({ path: resolve(evidence, "source-character-preview-contract-gap.png") });
    checks.push("complete native preview request retains unavailable base/equipment; no dummy loadout or preview substitution");
    await dismissNotices(page);
    await page.getByRole("button", { name: "Confirm appearance", exact: true }).click();
    if (earlyScene === null && recordedCamera === null) {
      await page.waitForFunction(() => ["world", "error"].includes(window.__clubscapeClientStateV1?.read().phase ?? ""), undefined, { timeout: 30_000 });
      const state = await page.evaluate(() => window.__clubscapeClientStateV1!.read());
      if (state.phase === "error") {
        assert(state.error?.errorId);
        assert.match(state.error?.message ?? "", /blocks cover .*source-bound live camera/);
        assert.equal(state.world, null);
        const benchmark = await page.evaluate(() => window.__clubscapeBenchmarkV1!.read(null));
        assert.equal(benchmark.renderedFrames, 0);
        await assertSourceRunPin(gameRoot, pin);
        await writeFile(resolve(evidence, "result.json"), JSON.stringify({
          kind: "actual-source-live-camera-boundary", result: "blocked", checks,
          sourceRunPin: pin, error: state.error, rendererCreated: true, automaticFixtureFallback: false,
          renderedFrames: benchmark.renderedFrames, fullJourneyTested: false, presentationAccepted: false,
        }, null, 2) + "\n");
        await rm(resolve(evidence, "failure.json"), { force: true });
        console.log(JSON.stringify({ result: "blocked", kind: "actual-source-live-camera-boundary",
          reason: state.error?.message, evidence: resolve(evidence, "result.json") }));
        process.exitCode = 2;
        return;
      }
    }
    await page.waitForFunction(() => {
      const state = window.__clubscapeClientStateV1?.read();
      return state?.phase === "world" && (state.world?.player as { appearanceConfirmed?: boolean } | undefined)?.appearanceConfirmed === true;
    }, undefined, { timeout: 30_000 });
    const first = await page.evaluate(() => window.__clubscapeClientStateV1!.read().world) as PublicWorld;
    assert(first.player.inventory.every((slot) => slot.item === null), "Source onboarding unexpectedly produced inventory.");
    assert.equal(first.player.questPoints, 0);
    assert.equal(first.player.tutorialStage, "stage.tutorial.experience");
    assert(first.player.presence?.connected);
    assert(!JSON.stringify(first).includes("played_time") && !JSON.stringify(first).includes("ground_provenance"));
    assert(Array.isArray(first.dynamicObjects));
    assert(first.dynamicObjects.every((object) => object.objectId === null ? object.sourceId === null
      : Number.isSafeInteger(object.sourceId) && object.sourceId! >= 0));
    assert(first.dynamicObjects.every((object) => object.expiresAtTick === null || /^\d+$/.test(object.expiresAtTick)));
    checks.push("empty source creation, real join and separate sequenced appearance confirmation from UI");
    const gameplayUi = await page.evaluate(() => window.__clubscapeClientStateV1!.gameplayUi());
    assert.equal(gameplayUi.available, false);
    assert.equal(gameplayUi.reason, "not_advertised");
    assert.equal(first.ui, undefined, "Contract publication cannot fabricate a version-1 server UI projection.");
    checks.push("legacy game.ui.v1 absence is explicitly unsupported; no fake complete UI state");
    const audioControls = await page.evaluate(() => window.__clubscapeClientStateV1!.audioControls());
    assert(audioControls);
    assert.equal(audioControls.semantics, "native-source-slider-v1");
    assert.equal(audioControls.masterPercent, sourceAudioDefaults().sliders.master);
    for (const channel of ["music", "effects", "area"] as const) {
      assert.equal(audioControls.channels[channel].percent, sourceAudioDefaults().sliders[channel]);
      assert.equal(audioControls.channels[channel].nativeMixer, sourceAudioDefaults().mixer[channel]);
    }
    assert.equal(audioControls.sourceSceneSupplied, false);
    assert.equal(audioControls.sourceMusicStateSupplied, false);
    assert.equal(audioControls.providedMusicState, null);
    assert.equal(audioControls.musicContinuation, "native-bound");
    checks.push("native audio defaults and internal music continuation observed; no source scene or manual unlock state invented");
    let renderPixels: unknown = null;
    if (earlyScene !== null || recordedCamera !== null) {
      await page.waitForFunction(() => (window.__clubscapeBenchmarkV1?.read(null).renderedFrames ?? 0) >= 8, undefined, { timeout: 30_000 });
      await dismissNotices(page);
      const screenshot = await page.locator("#world").screenshot({ path: resolve(evidence, recordedCamera ? "actual-streamed-source-world.png" : "actual-early-source-world.png") });
      const probe = execFileSync("python3", ["-B", "-c",
        "import io,json,sys;from PIL import Image;im=Image.open(io.BytesIO(sys.stdin.buffer.read())).convert('RGB'); p=list(im.getdata()); print(json.dumps({'width':im.width,'height':im.height,'nonblack':sum(max(v)>12 for v in p),'unique':len(set(p))}))",
      ], { cwd: root, input: screenshot, encoding: "utf8", maxBuffer: 1024 * 1024 });
      renderPixels = JSON.parse(probe) as { nonblack: number; unique: number };
      assert((renderPixels as { nonblack: number }).nonblack > 100_000);
      assert((renderPixels as { unique: number }).unique > 1000);
      checks.push(recordedCamera ? "real streamed source blocks at the authoritative region; nonblank hardware WebGPU with an explicitly recorded camera"
        : "explicit early fixture: actual nonblank WebGPU world composed with real UI/source actor state");
    }
    let timing: unknown = null;
    if (recordedCamera !== null) {
      const begin = await page.evaluate(() => window.__clubscapeBenchmarkV1!.read(null));
      await page.waitForTimeout(10_000);
      const end = await page.evaluate((cursor) => window.__clubscapeBenchmarkV1!.read(cursor), begin.renderedFrames) as ObservedSnapshot;
      const elapsed = end.nowMs - begin.nowMs;
      const frames = end.frames;
      assert.equal(frames.length, end.renderedFrames - begin.renderedFrames);
      assert(frames.length > 0);
      const completions = [begin.lastCompletedAtMs, ...frames.map((frame) => frame.completedAtMs)];
      const gaps = completions.slice(1).map((time, index) => time - completions[index]!);
      const tail = end.nowMs - end.lastCompletedAtMs;
      assert(end.diagnostics.loadedSquares?.includes(12336));
      assert.equal(end.diagnostics.scenePlacement?.blocks, true);
      assert.equal(end.diagnostics.scenePlacement?.sizeTiles, 104);
      assert([...renderRequests].some((path) => path.includes("/blocks/")));
      assert(![...renderRequests].some((path) => path.includes("/scenes/")), "recorded camera did not request a fixture scene");
      assert.equal(end.diagnostics.playerAnimationAvailable, false);
      const fps = frames.length * 1000 / elapsed;
      timing = {
        kind: "sparky-canonical-onboarding-engineering-only", measuredWindowMs: elapsed, completedFrames: frames.length,
        renderedFps: fps, completionGapsMs: distribution(gaps), trailingGapMs: tail,
        completionLatencyMs: distribution(frames.map((frame) => frame.completedAtMs - frame.submittedAtMs)),
        gpuPassMs: distribution(frames.flatMap((frame) => frame.gpuDurationMs === undefined ? [] : [frame.gpuDurationMs])),
        cpuEncodeMs: distribution(frames.flatMap((frame) => frame.cpuEncodeMs === undefined ? [] : [frame.cpuEncodeMs])),
        atLeast60Fps: fps >= 60, gapsOver33_4Ms: [...gaps, tail].filter((gap) => gap > 33.4).length,
        loadedSquares: end.diagnostics.loadedSquares, scenePlacement: end.diagnostics.scenePlacement,
        nativeScenePlacement: end.diagnostics.nativeScenePlacement,
        declaredWorkloadReady: end.ready, performanceAccepted: false, ownerHardware: false,
      };
      checks.push("per-world-frame performance.now receipts remain contiguous under the actual two-frame pipeline; metrics are engineering-only");
    }
    const progress = publicProgress(first);
    restarted = await restartSource(origin, pin, gameRoot);
    await page.waitForFunction((actorId) => {
      const state = window.__clubscapeClientStateV1?.read();
      return state?.phase === "world" && state.world?.player.id === actorId;
    }, first.player.id, { timeout: 35_000 });
    await page.waitForTimeout(1500);
    const after = await page.evaluate(() => window.__clubscapeClientStateV1!.read().world) as PublicWorld;
    assert.deepEqual(publicProgress(after), progress);
    assert(BigInt(after.tick) >= BigInt(first.tick));
    checks.push("actual same-artifact server restart/reconnect preserves acknowledged appearance/progress/possessions");
    await page.locator("#overlay").focus();
    await page.keyboard.press("Escape");
    await dismissNotices(page);
    await page.getByRole("button", { name: "Logout", exact: true }).first().click();
    await page.getByRole("button", { name: "Logout", exact: true }).last().click();
    await page.waitForFunction(() => window.__clubscapeClientStateV1?.read().phase === "title", undefined, { timeout: 20_000 });
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "Existing user", exact: true }).click();
    await fields(page, false);
    await page.getByRole("button", { name: "Login", exact: true }).click();
    await page.waitForFunction(() => window.__clubscapeClientStateV1?.read().phase === "world", undefined, { timeout: 30_000 });
    assert.deepEqual(publicProgress(await page.evaluate(() => window.__clubscapeClientStateV1!.read().world) as PublicWorld), progress);
    checks.push("actual UI logout/relogin resumes the same source character without repeating creation");
    const privacy = await page.evaluate(() => {
      const secret = (window as SecretWindow).__sourceUiCredentials!;
      const storage = JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } });
      const state = JSON.stringify(window.__clubscapeClientStateV1!.read());
      const safe = !storage.includes(secret.password) && !storage.includes(secret.name) && !state.includes(secret.password)
        && !/session_token|authorization_token/.test(state);
      Reflect.deleteProperty(window, "__sourceUiCredentials");
      return safe;
    });
    assert(privacy);
    assert.equal(unexpected.size, 0);
    assert.deepEqual([...failedAssets], [], "real source/component requests must not hide missing delivery assets");
    assert.equal(unhandledPageErrors, 0);
    checks.push("actual component asset routes have no failed responses or unhandled page errors");
    const projection = await page.evaluate(() => window.__clubscapePresentationV1);
    assert.equal(projection?.projection, "renderer-viewport-only-helper");
    assert.equal(projection?.fullHudProjectionMatched, false);
    const benchmark = await page.evaluate(() => window.__clubscapeBenchmarkV1!.read(null));
    assert.equal(benchmark.ready, false, "Unexposed actual entity counts cannot be treated as a complete benchmark workload.");
    if (earlyScene !== null || recordedCamera !== null) {
      assert(benchmark.renderedFrames > 0);
      assert(benchmark.lastSubmittedAtMs > 0 && benchmark.lastCompletedAtMs >= benchmark.lastSubmittedAtMs
        && benchmark.lastCompletedAtMs <= benchmark.nowMs);
      if (earlyScene) assert.equal(benchmark.identity.sceneId, earlyScene);
      else assert.match(benchmark.identity.sceneId, /^blocks@-?\d+,-?\d+$/);
      assert.equal(benchmark.identity.workloadId, earlyScene ? "early-presentation-not-journey" : "recorded-camera-not-journey");
      checks.push("GPU-completed frame records use actual canvas submission/receipt observations and correct performance.now clock");
    }
    await page.evaluate(() => {
      const device = (window as FaultWindow).__clubscapeFailureDevice;
      if (!device) throw new Error("The fault check did not observe the actual game-canvas device.");
      device.destroy();
      Reflect.deleteProperty(window, "__clubscapeFailureDevice");
    });
    await page.waitForFunction(() => {
      const state = window.__clubscapeClientStateV1!.read();
      return state.phase === "error" && state.error?.recoverable === false;
    }, undefined, { timeout: 10_000 });
    await page.waitForTimeout(250);
    const failedFrames = await page.evaluate(() => window.__clubscapeBenchmarkV1!.read(null).renderedFrames);
    await page.waitForTimeout(250);
    assert.equal(await page.evaluate(() => window.__clubscapeBenchmarkV1!.read(null).renderedFrames), failedFrames);
    assert.equal(await page.evaluate(() => window.__clubscapeBenchmarkV1!.read(null).ready), false);
    checks.push("actual game-device-loss fault check stops GPU frame publication and displays a terminal UI error; no fallback");
    await assertSourceRunPin(gameRoot, pin);
    await writeFile(resolve(evidence, "result.json"), JSON.stringify({
      kind: earlyScene ? "early-render-ui-canonical-source-entry" : "real-streamed-render-ui-canonical-source-entry",
      result: "passed", recordedAt: new Date().toISOString(),
      checks, gameplayUi, audioControls, sourceRunPin: pin, browser: version, sandbox: { namespaceAndSeccomp: true, gpuProcessSandboxed: system.gpu.auxAttributes?.sandboxed ?? null },
      titlePixels: title, build: benchmark.identity, rendererReady: benchmark.ready, renderedFrames: benchmark.renderedFrames,
      actualUiSignup: true, actualCanonicalWorld: true, actualServerRestart: true, actualDeviceLossHandled: true,
      earlyScene, recordedCamera, projection, renderPixels, preview, timing,
      dynamicObjects: { received: first.dynamicObjects.length, canonicalSourceMetadata: true },
      fullJourneyTested: false, worldRendererIntegrated: true,
      actualInitialBlocksLoaded: recordedCamera !== null, liveCameraSourceBound: false,
      regionStreamingComplete: false, presentationAccepted: false, milestoneAccepted: false,
    }, null, 2) + "\n");
    await rm(resolve(evidence, "failure.json"), { force: true });
    console.log(JSON.stringify({ result: "passed", kind: earlyScene ? "early-render-ui-canonical-source-entry" : "real-streamed-render-ui-canonical-source-entry",
      checks: checks.length, evidence: resolve(evidence, "result.json"), fullJourney: false }));
  } catch (error) {
    const diagnostic = await page?.evaluate(() => {
      const state = window.__clubscapeClientStateV1?.read();
      return { phase: state?.phase, error: state?.error, worldPresent: Boolean(state?.world),
        renderer: window.__clubscapeBenchmarkV1?.read(null),
        unlockedInterfaces: state?.world?.player.unlockedInterfaces,
        uiFeedback: document.querySelector('[data-clubscape-ui] [aria-live]')?.textContent,
        controls: Array.from(document.querySelectorAll<HTMLElement>("[data-ui-control]"), (element) => element.dataset.uiControl),
        inputs: Array.from(document.querySelectorAll<HTMLElement>("[data-ui-input]"), (element) => element.dataset.uiInput) };
    }).catch(() => null);
    await writeFile(resolve(evidence, "failure.json"), JSON.stringify({ kind: "source-ui-integration-failure", checksPassed: checks, diagnostic }, null, 2) + "\n");
    throw error;
  } finally {
    try { await context?.close(); }
    finally {
      try { if (restarted) await stopChild(restarted); }
      finally { await rm(runtime, { recursive: true, force: true }); }
    }
  }
}
