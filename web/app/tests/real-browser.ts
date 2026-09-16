import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import type { BrowserContext } from "playwright-core";
import type { WasmClient } from "../client.ts";
import { checkTitleAudio } from "./real-audio.ts";

export async function browserCheck(): Promise<void> {
  const root = resolve(fileURLToPath(new URL("../../../", import.meta.url)));
  const origin = process.env.CLUBSCAPE_BROWSER_ORIGIN;
  if (!origin || !/^http:\/\/127\.0\.0\.1:[1-9][0-9]{0,4}$/.test(origin)) {
    throw new Error("Set CLUBSCAPE_BROWSER_ORIGIN to the owned loopback account server's origin.");
  }
  if (!process.env.DISPLAY) throw new Error("Run this headful Sparky check under the documented Xvfb profile.");
  const output = resolve(root, process.env.CLUBSCAPE_BROWSER_EVIDENCE ?? ".local/evidence/browser-shell");
  if (!output.startsWith(root + sep)) throw new Error("Browser evidence must stay inside the worktree.");
  const executable = process.env.CLUBSCAPE_BROWSER_EXECUTABLE;
  if (!executable) throw new Error("Set CLUBSCAPE_BROWSER_EXECUTABLE to the verified native Chrome executable.");
  const version = execFileSync(executable, ["--version"], { encoding: "utf8" }).trim();
  assert.match(version, /Google Chrome (?:for Testing )?[0-9]+\./);
  await mkdir(output, { recursive: true, mode: 0o700 });
  // Chrome's singleton Unix socket must fit Linux's 108-byte sockaddr_un path.
  const runtime = resolve(root, ".local/c");
  assert(runtime.length + "/org.chromium.Chromium.XXXXXX/SingletonSocket".length < 108,
    "Choose a shorter worktree path for the native browser singleton socket.");
  await mkdir(runtime, { mode: 0o700 });
  const profile = resolve(output, "profile");
  const launchEnvironment: Record<string, string> = {};
  for (const key of ["PATH", "HOME", "DISPLAY", "XAUTHORITY", "LD_LIBRARY_PATH", "LANG"]) {
    if (process.env[key]) launchEnvironment[key] = process.env[key]!;
  }
  launchEnvironment.TMPDIR = runtime;
  launchEnvironment.TMP = runtime;
  launchEnvironment.TEMP = runtime;
  let context: BrowserContext | undefined;
  const checks: string[] = [];
  try {
    context = await chromium.launchPersistentContext(profile, {
      executablePath: executable, headless: false, chromiumSandbox: true,
      ignoreDefaultArgs: ["--enable-unsafe-swiftshader"],
      args: [
        "--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-angle=vulkan",
        "--enable-gpu", "--ignore-gpu-blocklist", "--ozone-platform=x11", "--enable-automation",
      ], env: launchEnvironment,
      viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1,
    });
    const browser = context.browser()!;
    const cdp = await browser.newBrowserCDPSession();
    const system = await cdp.send("SystemInfo.getInfo");
    const actualVersion = await cdp.send("Browser.getVersion");
    const command = await cdp.send("Browser.getBrowserCommandLine");
    const sandboxPage = await context.newPage();
    await sandboxPage.goto("chrome://sandbox");
    const sandbox = await sandboxPage.locator("body").innerText();
    assert(!command.arguments.some((argument) =>
      /^(--no-sandbox|--disable-.*sandbox|--disable-web-security|--single-process|--in-process-gpu|--no-zygote)(=|$)/.test(argument)));
    assert.match(sandbox, /Layer 1 Sandbox\s+Namespace/i);
    assert.match(sandbox, /PID namespaces\s+Yes/i);
    assert.match(sandbox, /Network namespaces\s+Yes/i);
    assert.match(sandbox, /Seccomp-BPF\s+sandbox\s+Yes/i);
    checks.push("actual Chrome namespace and seccomp sandbox verified");
    await sandboxPage.close();
    const page = await context.newPage();
    const requestPaths = new Set<string>();
    const external = new Set<string>();
    page.on("request", (request) => {
      const url = new URL(request.url());
      if (url.origin === origin) {
        if (url.search || url.hash || url.username || url.password) external.add("noncanonical URL");
        requestPaths.add(url.pathname);
      } else if (url.protocol !== "data:" && url.protocol !== "blob:") external.add(url.origin);
    });
    await page.goto(origin, { waitUntil: "networkidle" });
    await page.waitForFunction(() => document.querySelector("#bootstrap-status")?.getAttribute("role") === "alert"
      || document.querySelector<HTMLElement>("#bootstrap-status")?.hidden === true);
    const result = await page.evaluate(async () => {
      const path = "/client/bridge.js";
      const { BrowserApp, RpcTransport, createProtocolClient, checkCapability, AssetLoader, loadBuild, verifiedJson, parseContentManifest } =
        await import(path) as typeof import("../bridge.ts");
      const checks: string[] = [];
      const require: (condition: unknown, message: string) => asserts condition = (condition, message) => {
        if (!condition) throw new Error(message);
      };
      const wasm = await createProtocolClient() as WasmClient;
      const sourceUnavailable = async (): Promise<never> => { throw new Error("Infrastructure check must not invent source content/presentation."); };
      const app = new BrowserApp(wasm, new RpcTransport(), {
        content: sourceUnavailable, prepareWorld: sourceUnavailable,
        events() {}, unlockAudio: sourceUnavailable, volume() {}, disconnected() {},
        componentFailure(error) { app.report(error); },
      });
      const random = (): string => Array.from(crypto.getRandomValues(new Uint8Array(18)), (byte) => byte.toString(16).padStart(2, "0")).join("");
      const loginName = `browser_${random().slice(0, 10)}`;
      const password = random() + random();
      let capability: { available: boolean; failure: string | null };
      let sourceMetadata: { contentRevision: string; artifactSha256: string; manifestSha256: string; assets: number; decoded: number } | null = null;
      try { await checkCapability(); capability = { available: true, failure: null }; }
      catch (error) { capability = { available: false, failure: error instanceof Error ? error.message : "capability unavailable" }; }
      try {
        const build = await loadBuild();
        if (build.content) {
          const document = await verifiedJson(build.content.path, build.content.sha256, 8 * 1024 * 1024, fetch.bind(globalThis));
          const manifest = parseContentManifest(document.value);
          const assets = new AssetLoader(manifest, document.sha256);
          try {
            const item = Object.entries(manifest.catalog.items).find(([, item]) => item.asset !== null && item.sourceId !== null);
            const region = Object.values(manifest.regions)[0];
            require(item && item[1].asset && region, "Real source metadata selection is missing.");
            const definition = await assets.json(item[1].asset) as { id: number; interfaceOptions: Array<string | null> };
            require(definition.id === item[1].sourceId, "Published original item identity changed.");
            require(JSON.stringify(manifest.catalog.inventoryActions?.[item[0]])
              === JSON.stringify(definition.interfaceOptions.filter((name) => name !== null && name !== "")),
            "Inventory action labels are not the original source definition's labels.");
            const terrain = await assets.json(region.sceneAsset);
            require(terrain !== null && typeof terrain === "object" && Object.keys(terrain).length > 0,
              "Original region metadata did not decode.");
            const observed = assets.observe();
            require(observed.length === 2 && observed.every((asset) => asset.fetched && asset.decoded),
              "Source fetch/decode observations do not describe completed work.");
            sourceMetadata = { contentRevision: manifest.contentRevision, artifactSha256: manifest.artifactSha256,
              manifestSha256: document.sha256, assets: manifest.assets.length, decoded: observed.length };
            checks.push("real canonical ContentManifest and original item/region fetch/hash/decode");
            checks.push("inventory action labels retain original source definition values");
          } finally { assets.dispose(); }
        }
        await app.start();
        require(app.state().phase === "title", "Hello did not reach the title phase.");
        checks.push("real browser WASM initialization and binary hello");
        app.setScreen("register");
        await app.register(loginName, password);
        require(app.state().phase === "login", "Registration did not reach the login phase.");
        require(wasm.authorization() === undefined, "Registration fabricated an authenticated session.");
        checks.push("real persisted registration; no seeded character or automatic token");
        let duplicate = false;
        try { await app.register(loginName.toUpperCase(), password); }
        catch { duplicate = /^[0-9a-f-]{36}$/.test(app.state().error?.errorId ?? ""); }
        require(duplicate, "Duplicate normalized registration did not return a structured server error.");
        checks.push("duplicate registration and real error ID");
        let rejected = false;
        try { await app.login(loginName, "incorrect test password"); }
        catch { rejected = /^[0-9a-f-]{36}$/.test(app.state().error?.errorId ?? "") && wasm.authorization() === undefined; }
        require(rejected, "Wrong password did not reject authentication.");
        checks.push("wrong-password feedback without retained credential retry");
        await app.login(loginName, password);
        require(app.state().phase === "character" && app.state().accountName === loginName,
          "Real account login/current-account state was not recovered.");
        const state = JSON.parse(wasm.state()) as { gameplayAvailable: boolean; characterInitialized: boolean; contentRevision: string | null };
        require(!state.characterInitialized && app.state().world === null, "Account-only login fabricated a character/world.");
        checks.push("real login and current-account state; character remains uninitialized");
        if (!state.gameplayAvailable) {
          let refused = false;
          try { await app.enterWorld(); } catch { refused = app.state().world === null; }
          require(refused, "Unavailable world was presented as joined.");
          checks.push("account-only gameplay unavailability is explicit");
        } else {
          throw new Error("This bounded infrastructure runner requires an account-only server; use the separate source journey suite for game mode.");
        }
        wasm.transport_lost();
        await app.reconnect();
        require(app.state().accountName === loginName && app.state().world === null, "Memory-token account reconnect did not reconcile.");
        checks.push("real hello/current-account reconnect without reusing passwords");
        const token = wasm.authorization();
        require(token !== undefined, "Authenticated transport token missing.");
        const revokedRead = wasm.prepare(crypto.randomUUID(), "account", "{}");
        const publicState = JSON.stringify(app.state()) + wasm.state();
        require(!publicState.includes(password) && !publicState.includes(token!), "Public state exposed credentials.");
        require(wasm.state().includes('"world":null'), "Fixture produced a world.");
        await app.logout();
        require(wasm.authorization() === undefined && app.state().accountName === null, "Logout retained authentication.");
        const denied = await fetch("/v1/rpc", {
          method: "POST", body: new Uint8Array(revokedRead), headers: {
            "content-type": "application/x-protobuf", accept: "application/x-protobuf", authorization: `Bearer ${token}`,
          }, mode: "same-origin", credentials: "omit", redirect: "error", referrerPolicy: "no-referrer",
        });
        require(denied.status === 401, "Server did not revoke the actual bearer session.");
        revokedRead.fill(0);
        checks.push("real logout, memory clearing, and server-side token revocation");
        await app.login(loginName, password);
        require(app.state().accountName === loginName, "Persisted account could not log in again.");
        await app.logout();
        checks.push("persisted account relogin and final revocation");
        const storage = JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } });
        require(!storage.includes(password) && !storage.includes(token!) && !storage.includes(loginName),
          "Credentials or account identity were persisted in browser storage.");
        checks.push("public-state and browser-storage credential/privacy boundaries");
        const benchmark = window.__clubscapeBenchmarkV1?.read(null);
        require(benchmark?.ready === false && benchmark.renderedFrames === 0,
          "Missing product renderer was incorrectly presented as a completed frame workload.");
        const firstFrames = benchmark.renderedFrames;
        for (let index = 0; index < 20; index++) require(window.__clubscapeBenchmarkV1?.read(null).renderedFrames === firstFrames, "read() advanced counters.");
        checks.push("benchmark read-only and absent-renderer readiness/counters remain false/zero");
        return {
          checks, capability, sourceMetadata, crossOriginIsolated,
          userAgent: navigator.userAgent,
          build: benchmark.identity, benchmarkReady: benchmark.ready, completedRendererFrames: benchmark.renderedFrames,
          visibleBootstrapDiagnostic: document.querySelector("#bootstrap-status")?.textContent,
        };
      } finally { await app.dispose(); }
    });
    checks.push(...result.checks);
    const titleAudio = await checkTitleAudio(page);
    if (titleAudio !== null) checks.push("actual source audio factory + shell trusted gesture/title/disconnect composition");
    assert.equal(result.crossOriginIsolated, true);
    assert.equal(external.size, 0, "No credentials or transport may escape the same-origin public routes.");
    await page.screenshot({ path: resolve(output, "bootstrap-integration-diagnostic.png"), fullPage: false });
    await cdp.detach();
    await writeFile(resolve(output, "result.json"), JSON.stringify({
      schemaVersion: 1, kind: "real-browser-wasm-account-infrastructure-fixture",
      recordedAt: new Date().toISOString(), result: "passed", checks,
      browser: { executableVersion: version, actualVersion, userAgent: result.userAgent },
      profile: "sparky-vulkan-x11; headful Xvfb1280x800/DPR1; not physical presentation",
      sandbox: { namespaceAndSeccompVerified: true, gpuProcessSandboxed: system.gpu.auxAttributes?.sandboxed ?? null },
      build: result.build, capability: result.capability, requestPaths: [...requestPaths].sort(),
      sourceMetadata: result.sourceMetadata,
      titleAudio,
      benchmarkReady: result.benchmarkReady, completedRendererFrames: result.completedRendererFrames,
      screenshot: "bootstrap-integration-diagnostic.png",
      screenshotSha256: createHash("sha256").update(await readFile(resolve(output, "bootstrap-integration-diagnostic.png"))).digest("hex"),
      visibleBootstrapDiagnostic: result.visibleBootstrapDiagnostic,
      realUiSignupTested: false, rendererUiAudioIntegrated: false, gameplayAccepted: false, presentationAccepted: false,
      macChromeEdgeAcceptance: "not run",
    }, null, 2) + "\n");
    console.log(JSON.stringify({ result: "passed", kind: "real-browser-wasm-account-infrastructure-fixture", checks: checks.length, evidence: resolve(output, "result.json"), gameJourney: false }));
  } finally {
    await context?.close();
    await rm(runtime, { recursive: true, force: true });
  }
}
