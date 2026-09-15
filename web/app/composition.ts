import type { RendererHandle, UiHandle, WorldView } from "../shared/contracts.ts";
import { SOURCE_PACK_SHA256 } from "../shared/contracts.ts";
import { AssetLoader } from "./assets.ts";
import type { BuildConfig } from "./build.ts";
import { verifiedJson } from "./build.ts";
import { Benchmark } from "./benchmark.ts";
import type { RendererObservation } from "./benchmark.ts";
import { checkCapability, watchCanvasDevice } from "./capability.ts";
import { BrowserApp } from "./client.ts";
import type { WasmClient } from "./client.ts";
import type { Components } from "./components.ts";
import { AppError, appError, invariant } from "./errors.ts";
import { InputController } from "./input.ts";
import { canonicalJson } from "./identity.ts";
import { parseContentManifest } from "./manifest.ts";
import { Settings } from "./settings.ts";
import { RpcTransport } from "./transport.ts";
import { presenceOf } from "./public-state.ts";
import { SourceAudioSession, audioProblem, playbackEnabled, sourceAudioAdapter } from "./audio.ts";

export interface ObservedRenderer extends RendererHandle {
  /** Observation only; all values must come from the real decoder/render path. */
  observe?(): RendererObservation;
}
export interface ApplicationHandle { app: BrowserApp; dispose(): Promise<void> }

export async function mountApplication(options: {
  build: BuildConfig;
  components: Components;
  bridge: WasmClient;
  benchmark: Benchmark;
  worldCanvas: HTMLCanvasElement;
  uiCanvas: HTMLCanvasElement;
  status: HTMLElement;
}): Promise<ApplicationHandle> {
  const { build, components, bridge, benchmark, worldCanvas, uiCanvas, status } = options;
  let ui: UiHandle | null = null;
  let audio: SourceAudioSession | null = null;
  let renderer: ObservedRenderer | null = null;
  let input: InputController | null = null;
  let assets: AssetLoader | null = null;
  let required: string[] = [];
  let frame = 0;
  let priorFrameMs = performance.now();
  let inFlight = 0;
  let sceneLoaded = false;
  let disposed = false;
  let stopDevice: (() => void) | null = null;
  let unsubscribe: (() => void) | null = null;
  let resize: ResizeObserver | null = null;
  let hashGeneration = 0;
  let componentFailed = false;
  let appliedRenderSettings: Readonly<Record<string, unknown>> | null = null;
  let appliedRenderSettingsJson = "";
  const lifecycle = new AbortController();
  const transport = new RpcTransport();
  let storage: Storage | null;
  try { storage = localStorage; } catch { storage = null; }
  const settings = new Settings(storage);
  const observeAssets = (): void => {
    const observed = new Map([...(assets?.observe() ?? []), ...(audio?.observations() ?? [])].map((asset) => [asset.id, asset]));
    benchmark.assets([...observed.values()]);
  };

  async function settingsHash(): Promise<void> {
    const generation = ++hashGeneration;
    benchmark.settings("");
    if (appliedRenderSettings === null) return;
    const hash = await settings.hash({ declaredProfile: build.visualSettings, renderer: appliedRenderSettings });
    if (!disposed && generation === hashGeneration) benchmark.settings(hash);
  }

  const app = new BrowserApp(bridge, transport, {
    async content(revision, path) {
      invariant(assets && build.content && path === build.content.path
        && revision === assets.manifest.contentRevision,
      "The server world does not match this build's pinned public source content. Rebuild against the current content manifest.", "integration");
      return assets.manifest.catalog;
    },
    async prepareWorld(world: WorldView) {
      invariant(renderer && assets, "The real renderer/source loader is not ready.", "integration");
      const region = assets.manifest.regions[world.player.region];
      invariant(region, `No compiled source presentation exists for ${world.player.region}.`, "integration");
      invariant(region.camera && region.controls, "This region has no recorded source camera/input binding.", "integration");
      sceneLoaded = false;
      benchmark.worldReady(false);
      required = Array.from(new Set([...assets.manifest.bootstrap, ...region.requiredAssets,
        ...(assets.manifest.rendererManifest ? [assets.manifest.rendererManifest] : [])]));
      assets.retain(required);
      benchmark.scene(region.sceneId, region.routeId, region.workloadId, assets.manifestSha256,
        new Map(required.map((id) => [id, assets!.manifest.assets.find((asset) => asset.id === id)!.sha256])));
      await assets.preload(required);
      renderer.update(world);
      await renderer.loadScene(region.sceneId);
      input?.dispose();
      input = new InputController(uiCanvas, ui!, renderer, app, region.camera, region.controls, world.player.tile);
      sceneLoaded = true;
    },
    events(world, events) {
      audio?.update(world, events);
    },
    async unlockAudio() {
      if (!audio) throw new AppError("The source audio component is not ready.", { kind: "audio" });
      await audio.unlock();
    },
    audioEnabled() { return audio?.enabled() === true; },
    volume(channel, value) {
      const bounded = settings.volume(channel, value);
      audio?.volume(channel, bounded);
      void settingsHash().catch(() => app.report(new AppError("Settings identity could not be calculated.", { kind: "benchmark" })));
    },
    disconnected() {
      benchmark.worldReady(false);
      audio?.disconnected();
    },
  });

  const dispose = async (): Promise<void> => {
    if (disposed) return;
    disposed = true;
    cancelAnimationFrame(frame);
    lifecycle.abort();
    resize?.disconnect();
    unsubscribe?.();
    input?.dispose();
    stopDevice?.();
    renderer?.dispose();
    ui?.dispose();
    assets?.dispose();
    await app.dispose();
    await audio?.dispose();
  };

  try {
    const capability = checkCapability().then(() => null, (error: unknown) => appError(error, "WebGPU capability could not be established."));
    await settingsHash();
    invariant(build.content, "No compiled source ContentManifest is configured for this build. Set CLUBSCAPE_CLIENT_MANIFEST and rebuild; no replacement UI/assets were loaded.", "integration");
    const manifest = await verifiedJson(build.content.path, build.content.sha256, 8 * 1024 * 1024, globalThis.fetch.bind(globalThis));
    assets = new AssetLoader(parseContentManifest(manifest.value), manifest.sha256, {
      changed() {
        if (!assets) return;
        const counts = assets.counts(required);
        app.loading(counts.fetched, counts.total, `Source assets: ${counts.fetched}/${counts.total} fetched, ${counts.decoded} decoded`);
        observeAssets();
      },
    });
    required = [...assets.manifest.bootstrap];
    await assets.preload(required);
    ui = await components.createUi(uiCanvas, app, assets);
    unsubscribe = app.subscribe((state) => {
      if (componentFailed) return;
      try {
        ui?.update(state);
        if (state.world && renderer && sceneLoaded) renderer.update(state.world);
        const presence = presenceOf(state.world);
        benchmark.worldReady(state.phase === "world" && sceneLoaded && presence?.connected === true && presence.presentInWorld);
      } catch {
        componentFailed = true;
        sceneLoaded = false;
        status.hidden = false;
        status.textContent = "The source presentation component failed. Reload after checking its build and source assets.";
        queueMicrotask(() => app.report(new AppError("The source UI/renderer could not apply the authoritative view.", { kind: "component", recoverable: false })));
      }
    });
    status.hidden = true;
    const resizeSurfaces = (): void => {
      const width = Math.max(1, Math.round(worldCanvas.clientWidth));
      const height = Math.max(1, Math.round(worldCanvas.clientHeight));
      const scale = window.devicePixelRatio;
      const pixelsWide = Math.round(width * scale);
      const pixelsHigh = Math.round(height * scale);
      if (worldCanvas.width !== pixelsWide || worldCanvas.height !== pixelsHigh) {
        worldCanvas.width = pixelsWide; worldCanvas.height = pixelsHigh;
      }
      if (uiCanvas.width !== pixelsWide || uiCanvas.height !== pixelsHigh) {
        uiCanvas.width = pixelsWide; uiCanvas.height = pixelsHigh;
      }
      renderer?.resize(pixelsWide, pixelsHigh);
      ui?.resize(pixelsWide, pixelsHigh);
      benchmark.viewport(width, height, scale);
    };
    resizeSurfaces();
    resize = new ResizeObserver(resizeSurfaces);
    resize.observe(worldCanvas);
    window.addEventListener("resize", resizeSurfaces, { signal: lifecycle.signal });
    const capabilityError = await capability;
    if (capabilityError) {
      app.report(capabilityError);
      return { app, dispose };
    }
    invariant(assets.manifest.rendererManifest, "The public source manifest has no renderer adapter manifest.", "integration");
    stopDevice = watchCanvasDevice(worldCanvas, (epoch, ready, timestamps) => benchmark.device(epoch, ready, timestamps), (error) => {
      sceneLoaded = false;
      benchmark.worldReady(false);
      audio?.disconnected();
      app.report(error);
    });
    renderer = await components.createRenderer(worldCanvas, {
      assetBaseUrl: assets.baseUrl, manifestUrl: assets.url(assets.manifest.rendererManifest),
      sourcePackSha256: SOURCE_PACK_SHA256, width: worldCanvas.width, height: worldCanvas.height,
    });
    audio = await SourceAudioSession.create(assets, assets.manifest.assets,
      (error) => app.report(audioProblem(error)),
      (state) => {
        app.audioStatus(playbackEnabled(state));
        benchmark.audio(state);
        observeAssets();
      }, { ...sourceAudioAdapter, create: components.createAudio });
    const overrides = settings.audioOverrides();
    for (const channel of ["music", "effects", "area"] as const) {
      if (overrides[channel] !== undefined) audio.volume(channel, overrides[channel]);
    }
    audio.update(null, []);
    const render = (now: number): void => {
      if (disposed) return;
      frame = requestAnimationFrame(render);
      const delta = now - priorFrameMs;
      priorFrameMs = now;
      if (!renderer || !sceneLoaded || app.state().phase !== "world") return;
      try {
        input?.update(delta);
        invariant(inFlight < 128, "The renderer has too many uncompleted GPU submissions.", "device");
        inFlight++;
        // frame() resolves on GPU completion. Do not serialize submissions behind receipts.
        void renderer.frame(now).then((completed) => {
          if (disposed) return;
          if (completed !== null) benchmark.completed(completed);
          const observation = renderer?.observe?.() ?? null;
          benchmark.renderer(observation);
          const settingsJson = observation?.settings ? canonicalJson(observation.settings) : "";
          if (settingsJson !== appliedRenderSettingsJson) {
            appliedRenderSettingsJson = settingsJson;
            appliedRenderSettings = observation?.settings ?? null;
            void settingsHash().catch(() => app.report(new AppError("Applied render settings could not be hashed.", { kind: "benchmark", recoverable: false })));
          }
        }).catch(() => {
          sceneLoaded = false;
          benchmark.worldReady(false);
          app.report(new AppError("The real renderer failed to complete or observe its GPU frame.", { kind: "device", recoverable: false }));
        }).finally(() => { inFlight--; });
      } catch (error) {
        sceneLoaded = false;
        app.report(appError(error, "The source game renderer failed."));
      }
    };
    frame = requestAnimationFrame(render);
    window.addEventListener("online", () => {
      if (app.state().phase === "reconnecting") void app.reconnect().catch(() => {});
    }, { signal: lifecycle.signal });
    window.addEventListener("offline", () => transport.abort(), { signal: lifecycle.signal });
    await app.start();
    return { app, dispose };
  } catch (value) {
    const error = appError(value);
    if (ui) {
      app.report(error);
      return { app, dispose };
    }
    await dispose();
    throw error;
  }
}
