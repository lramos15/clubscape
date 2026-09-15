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
import { sourceZoomForViewportHeight } from "./renderer.ts";
import type { AudioSnapshot, SourceAudioScene, SourceMusicState } from "../audio/index.ts";
import type { UiPreviewRequest } from "../ui/index.ts";
import { ModelPreview } from "./preview.ts";
import { sourceUiAudioAdapter, sourceUiPreviewAdapter } from "./ui-adapter.ts";

export interface ObservedRenderer extends RendererHandle {
  /** Observation only; all values must come from the real decoder/render path. */
  observe?(): RendererObservation;
  supportsScene?(id: string): boolean;
  frameUiPreview?(request: Readonly<UiPreviewRequest>): Promise<ImageData | null>;
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
  earlyScene?: string | null;
  recordedCamera?: string | null;
  sourceAudio?: {
    scene?(world: WorldView): SourceAudioScene | undefined;
    music?(world: WorldView): SourceMusicState | undefined;
  };
}): Promise<ApplicationHandle> {
  const { build, components, bridge, benchmark, worldCanvas, uiCanvas, status } = options;
  const earlyScene = options.earlyScene ?? null;
  const recordedCamera = options.recordedCamera ?? null;
  const presentationCamera = earlyScene ?? recordedCamera;
  const diagnosticWorkload = earlyScene ? "early-presentation-not-journey" : recordedCamera ? "recorded-camera-not-journey" : null;
  let ui: UiHandle | null = null;
  let audio: SourceAudioSession | null = null;
  let renderer: ObservedRenderer | null = null;
  let preview: ModelPreview | null = null;
  let appliedWorld: WorldView | null = null;
  let rendererHadWorld = false;
  let previewResetReported = false;
  let input: InputController | null = null;
  let assets: AssetLoader | null = null;
  let required: string[] = [];
  let assetPins = new Map<string, string>();
  let requiredPins = new Map<string, string>();
  let frame = 0;
  let priorFrameMs = performance.now();
  let inFlight = 0;
  let sceneLoaded = false;
  let disposed = false;
  let stopDevice: (() => void) | null = null;
  let unsubscribe: (() => void) | null = null;
  let stopAudioUi: (() => void) | null = null;
  let stopMusicPreferences: (() => void) | null = null;
  let resize: ResizeObserver | null = null;
  let hashGeneration = 0;
  let componentFailed = false;
  let appliedRenderSettings: Readonly<Record<string, unknown>> | null = null;
  let appliedRenderSettingsJson = "";
  let appliedAudioSettings: Pick<AudioSnapshot, "masterPercent" | "volumes" | "nativeMixer" | "muted"> | null = null;
  let appliedSettingsKey = "";
  let settingsActor: string | null = null;
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
    if (disposed || appliedRenderSettings === null) return;
    const source = ui ? sourceUiAudioAdapter.readMusic(ui) : null;
    const music = source ? { mode: source.mode, areaMode: source.areaMode, selectedGroup: source.selectedGroup,
      playlistGroups: [...source.playlistGroups], loopEnabled: source.loopEnabled } : null;
    const visual = { declaredProfile: build.visualSettings, renderer: appliedRenderSettings, audio: appliedAudioSettings, music };
    const key = canonicalJson({ visual, preferences: settings.read(), overrides: settings.audioOverrides() });
    if (key === appliedSettingsKey) return;
    appliedSettingsKey = key;
    const generation = ++hashGeneration;
    benchmark.settings("");
    const hash = await settings.hash(visual);
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
      invariant(assets, "The validated source loader is not ready.", "integration");
      const region = assets.manifest.regions[world.player.region];
      invariant(region, `No compiled source presentation exists for ${world.player.region}.`, "integration");
      sceneLoaded = false;
      benchmark.worldReady(false);
      const fixture = presentationCamera === null ? null : assets.manifest.renderer?.fixtures[presentationCamera];
      if (presentationCamera !== null && !fixture) throw new AppError("The explicitly requested source fixture/camera is not exported.", { kind: "region_unavailable" });
      const sceneId = earlyScene ?? region.sceneId;
      const camera = fixture?.camera ?? region.camera;
      if (!renderer || !renderer.supportsScene?.(sceneId)) {
        throw new AppError(`The actual renderer has no published block coverage for authoritative region ${world.player.region}. No arbitrary scene or blank fallback was selected.`, { kind: "region_unavailable" });
      }
      if (camera === null) throw new AppError(`Published renderer blocks cover ${world.player.region}, but a source-bound live camera is not supplied. Explicit recorded-camera presentation is separate; no spawn-camera defaults were invented.`, { kind: "camera_unavailable" });
      if (recordedCamera !== null) {
        const coverage = assets.manifest.renderer;
        const square = Math.floor(camera.x / (64 * 128)) * 256 + Math.floor(camera.y / (64 * 128));
        invariant(coverage?.coverage === "source_world_blocks" && coverage.regions[world.player.region]?.square === square,
          "The explicitly recorded camera is not in this authoritative source region.", "camera_unavailable");
      }
      required = Array.from(new Set([...assets.manifest.bootstrap, ...(earlyScene !== null ? fixture!.requiredAssets : region.requiredAssets),
        ...(assets.manifest.renderer?.coverage === "source_world_blocks" ? assets.manifest.renderer.commonAssets : []),
        ...(assets.manifest.rendererManifest ? [assets.manifest.rendererManifest] : [])]));
      requiredPins = new Map(required.map((id) => [id, assetPins.get(id)!]));
      const uiAssets = Object.entries(assets.manifest.aliases ?? {}).filter(([id]) => id.startsWith("ui/")).map(([, id]) => id);
      assets.retain([...required, ...uiAssets]);
      benchmark.scene(sceneId, earlyScene ? `early.presentation.${sceneId}` : region.routeId,
        diagnosticWorkload ?? region.workloadId,
        assets.manifest.renderer?.manifestSha256 ?? assets.manifestSha256,
        requiredPins);
      await assets.preload(required);
      await renderer.loadScene(sceneId);
      renderer.update(world);
      appliedWorld = world;
      rendererHadWorld = true;
      input?.dispose();
      input = new InputController(uiCanvas, ui!, renderer, app, { ...camera, zoom: sourceZoomForViewportHeight(worldCanvas.height) },
        fixture ? null : region.controls, world.player.tile,
        () => ({ width: worldCanvas.width, height: worldCanvas.height }));
      sceneLoaded = true;
    },
    events(world, events) {
      let scene: SourceAudioScene | null | undefined;
      let music: SourceMusicState | undefined;
      try { scene = world === null ? null : options.sourceAudio?.scene?.(world); }
      catch (error) { app.report(audioProblem(error)); }
      try { music = world === null ? undefined : options.sourceAudio?.music?.(world); }
      catch (error) { app.report(audioProblem(error)); }
      audio?.update(world, events, scene, music);
    },
    async unlockAudio() {
      if (!audio) throw new AppError("The source audio component is not ready.", { kind: "audio" });
      await audio.unlock();
    },
    audioEnabled() { return audio?.enabled() === true; },
    audioControls() { return audio?.controls() ?? null; },
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
    preview?.dispose();
    renderer?.dispose();
    stopDevice?.();
    stopMusicPreferences?.();
    stopAudioUi?.();
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
    assetPins = new Map(assets.manifest.assets.map((asset) => [asset.id, asset.sha256]));
    required = [...assets.manifest.bootstrap];
    await assets.preload(required);
    ui = await components.createUi(uiCanvas, app, assets);
    unsubscribe = app.subscribe((state) => {
      if (componentFailed) return;
      try {
        // createUi owns its state subscription; this observer drives only the renderer/benchmark.
        if (state.world && state.world !== appliedWorld && renderer && sceneLoaded) {
          renderer.update(state.world);
          appliedWorld = state.world;
          rendererHadWorld = true;
        }
        const presence = presenceOf(state.world);
        benchmark.worldReady(state.phase === "world" && sceneLoaded && presence?.connected === true && presence.presentInWorld
          && app.gameplayUi().available);
        const actor = state.world?.player.id ?? null;
        if (actor !== settingsActor) {
          settingsActor = actor;
          void settingsHash().catch(() => app.report(new AppError("Player-scoped applied settings could not be hashed.", { kind: "benchmark" })));
        }
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
      renderer?.resize(pixelsWide, pixelsHigh);
      input?.resizeZoom(sourceZoomForViewportHeight(pixelsHigh));
      ui?.resize(width, height);
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
    if (components.createRenderer && assets.manifest.rendererManifest) {
      worldCanvas.addEventListener("clubscape-render-diagnostic", (event) => {
        const message = (event as CustomEvent<unknown>).detail;
        if (typeof message === "string") app.report(new AppError(message, { kind: "renderer" }));
      }, { signal: lifecycle.signal });
      stopDevice = watchCanvasDevice(worldCanvas, (epoch, ready, timestamps) => benchmark.device(epoch, ready, timestamps), (error) => {
        sceneLoaded = false;
        benchmark.worldReady(false);
        audio?.disconnected();
        app.report(error);
      });
      renderer = await components.createRenderer(worldCanvas, {
        assetBaseUrl: assets.manifest.renderer?.assetBaseUrl ?? assets.baseUrl, manifestUrl: assets.url(assets.manifest.rendererManifest),
        sourcePackSha256: SOURCE_PACK_SHA256, width: worldCanvas.width, height: worldCanvas.height,
      });
      const previewRenderer = renderer;
      if (previewRenderer.frameUiPreview) {
        preview = new ModelPreview({
          request: () => sourceUiPreviewAdapter.request(ui!),
          frame: (request) => previewRenderer.frameUiPreview!(request),
          publish: (surface) => sourceUiPreviewAdapter.publish(ui!, surface),
          report: (error) => app.report(error),
        });
      }
    }
    audio = await SourceAudioSession.create(assets, assets.manifest.assets,
      (error) => app.report(audioProblem(error)),
      (state) => {
        appliedAudioSettings = { masterPercent: state.masterPercent, volumes: { ...state.volumes },
          nativeMixer: { ...state.nativeMixer }, muted: state.muted };
        app.audioStatus(playbackEnabled(state));
        benchmark.audio(state);
        observeAssets();
        void settingsHash().catch(() => app.report(new AppError("Observed native audio settings could not be hashed.", { kind: "benchmark" })));
      }, {
        ...sourceAudioAdapter, create: components.createAudio,
        musicState(_handle, state) {
          const world = app.state().world;
          invariant(ui && world, "Source music state requires the actual current player and UI binding.", "audio");
          sourceUiAudioAdapter.music(ui, world.player.id, state);
          void settingsHash().catch(() => app.report(new AppError("Applied music settings could not be hashed.", { kind: "benchmark" })));
        },
        readMusicState() { return ui ? sourceUiAudioAdapter.readMusic(ui) : null; },
      });
    stopAudioUi = await audio.bindUi((handle) => sourceUiAudioAdapter.bind(ui!, handle));
    stopMusicPreferences = sourceUiAudioAdapter.musicChanges(ui, (playerId, state) => {
      invariant(app.state().world?.player.id === playerId
        && canonicalJson(sourceUiAudioAdapter.readMusic(ui!)) === canonicalJson(state),
      "Applied music preferences no longer belong to the current player snapshot.", "audio_preferences");
      void settingsHash().catch(() => app.report(new AppError("Applied music preferences could not be hashed.", { kind: "benchmark" })));
      throw new AppError("Music preferences were applied for this player, but persistent saving is unavailable until the audio owner's versioned client-preference helper is integrated. No unlocks or guessed restored values were stored.",
        { kind: "audio_preferences", errorId: "audio.preferences.persistence_unavailable" });
    });
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
      try {
        const state = app.state();
        const previewOwner = state.world ? canonicalJson({
          actor: state.world.player.id, appearance: state.world.player.appearance, equipment: state.world.player.equipment,
        }) : state.phase === "character" && !rendererHadWorld ? "uncreated-source-body" : null;
        preview?.update(previewOwner);
        if (preview) benchmark.preview(preview.observe());
        if (state.phase === "character" && rendererHadWorld && !previewResetReported) {
          previewResetReported = true;
          app.report(new AppError("A new character preview needs a renderer actor reset after the preceding session; prior character equipment is not reused.", { kind: "renderer_preview" }));
        }
        if (!renderer || !sceneLoaded || state.phase !== "world") return;
        input?.update(delta);
        invariant(inFlight < 128, "The renderer has too many uncompleted GPU submissions.", "device");
        inFlight++;
        // frame() resolves on GPU completion. Do not serialize submissions behind receipts.
        void renderer.frame(now).then((completed) => {
          if (disposed) return;
          if (completed !== null) benchmark.completed(completed);
          const observation = renderer?.observe?.() ?? null;
          const delivery = assets?.manifest.renderer;
          const observedAssets = observation?.assets.flatMap((asset) => {
            const id = delivery?.assetIds[asset.id];
            return id ? [{ ...asset, id }] : [];
          }) ?? [];
          if (observation && assets && delivery) {
            const world = app.state().world;
            const region = world ? assets.manifest.regions[world.player.region] : undefined;
            const activeAssets = new Map(requiredPins);
            for (const asset of observedAssets) if (asset.loaded) activeAssets.set(asset.id, asset.sha256);
            benchmark.scene(observation.sceneId, earlyScene ? `early.presentation.${earlyScene}` : region?.routeId ?? "unavailable",
              diagnosticWorkload ?? region?.workloadId ?? "unavailable",
              delivery.manifestSha256, activeAssets);
          }
          benchmark.renderer(observation ? { ...observation, assets: observedAssets } : null);
          const settingsJson = observation?.settings ? canonicalJson(observation.settings) : "";
          if (settingsJson !== appliedRenderSettingsJson) {
            appliedRenderSettingsJson = settingsJson;
            appliedRenderSettings = observation?.settings ?? null;
            void settingsHash().catch(() => app.report(new AppError("Applied render settings could not be hashed.", { kind: "benchmark", recoverable: false })));
          }
        }).catch((error: unknown) => {
          if (disposed) return;
          sceneLoaded = false;
          benchmark.worldReady(false);
          app.report(error instanceof AppError ? error
            : new AppError("The real renderer failed to complete or observe its GPU frame.", { kind: "device", recoverable: false }));
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
    const migration = settings.migrationNotice();
    if (migration) app.report(migration);
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
