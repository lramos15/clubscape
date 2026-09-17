import { createRenderer, fullHudViewport, fullHudZoomForViewport, regionSceneId, sourceZoomForViewportHeight } from "../renderer/src/index.ts";
import type { ClubscapeRendererHandle, MapIconSprite, MinimapIconPlacements, MinimapSurface, PlayerFitReport, PlayerPoseFit, PlayerPreviewRequest, RenderAssetManifest, RendererDiagnostics, RendererWorldExtensions, ScenePlacement, UnboundAction } from "../renderer/src/index.ts";
import wasmUrl from "../renderer/pkg/clubscape_renderer_bg.wasm?url";
import type { RenderCamera, RendererConfig, RenderFrame, RendererHandle, WorldView } from "../shared/contracts.ts";
import type { RendererObservation } from "./benchmark.ts";
import { verifiedJson } from "./build.ts";
import { CanvasGpuClock } from "./gpu-clock.ts";
import { AppError, invariant } from "./errors.ts";
import { publicPath } from "./identity.ts";
import { canonicalPick } from "./picking.ts";
import { RENDER_MANIFEST_SHA256 } from "./render-identity.ts";
import { residentRendererAssets, sourceScenePlacement } from "./render-state.ts";
import { PlayerModelPreviewProducer } from "./ui-preview-request.ts";
import type { UiPreviewRequest } from "../ui/index.ts";
import type { NativeCameraRenderer } from "../renderer/src/camera.ts";

export { fullHudViewport, fullHudZoomForViewport, regionSceneId, sourceZoomForViewportHeight };

export interface ShellRenderer extends RendererHandle, NativeCameraRenderer {
  observe(): RendererObservation;
  diagnostics(): RendererDiagnostics;
  supportsScene(id: string): boolean;
  framePlayerPreview(request: PlayerPreviewRequest): Promise<ImageData | null>;
  frameUiPreview(request: Readonly<UiPreviewRequest>, world: WorldView | null): Promise<ImageData | null>;
  clearPreviewMetadata(): void;
  playerFitReport(): PlayerFitReport[];
  scenePlacement(): ScenePlacement | null;
  minimapSurface(): MinimapSurface;
  mapIconSprites(): Map<number, MapIconSprite>;
  playerPoseFits(): PlayerPoseFit[];
  observerV1(): boolean;
  playerRunning(): boolean;
  unknownMotions(): string[];
  unboundActions(): UnboundAction[];
  minimapIconPlacements(playerTileX: number, playerTileY: number, scale: number, width: number, height: number): MinimapIconPlacements;
}

/** Exact adapter composition: real factory, real diagnostics, and the actual canvas queue clock. */
export async function createShellRenderer(canvas: HTMLCanvasElement, config: RendererConfig): Promise<ShellRenderer> {
  publicPath(config.manifestUrl);
  publicPath(`${config.assetBaseUrl.replace(/\/$/, "")}/manifest.json`);
  const { value } = await verifiedJson(config.manifestUrl, RENDER_MANIFEST_SHA256, 2 * 1024 * 1024, fetch.bind(globalThis));
  const manifest = value as RenderAssetManifest;
  const sceneIds = new Set([...manifest.scenes.map((scene) => scene.name),
    ...(manifest.blocks?.map((block) => regionSceneId(block.square)) ?? [])]);
  const clock = new CanvasGpuClock(canvas);
  let native: ClubscapeRendererHandle;
  const reported = new Set<string>();
  const report = (message: string) => {
    if (reported.has(message)) return;
    reported.add(message);
    canvas.dispatchEvent(new CustomEvent("clubscape-render-diagnostic", { detail: message }));
  };
  try {
    native = await createRenderer(canvas, config, {
      wasmUrl, onDiagnostic: report, maxFramesInFlight: 2, developerMotionFallback: false,
    });
  } catch (error) {
    clock.dispose();
    throw new AppError(`Actual WebGPU renderer initialization failed: ${String(error)}`, { kind: "device", recoverable: false });
  }
  let camera: RenderCamera | null = null;
  let world: WorldView | null = null;
  let instanceTemplate: string | null = null;
  let disposed = false;
  let lastFrame: RenderFrame | null = null;
  let nativeCameraMode = false;
  let nativeCameraCurrent = false;
  const cameraCall = <T>(call: () => T): T => {
    try { return call(); }
    catch (error) {
      if (error instanceof AppError) throw error;
      const detail = typeof error === "string" ? error : error instanceof Error ? error.message : "No native diagnostic.";
      throw new AppError(`Source camera provider failed: ${detail.slice(0, 2048)}`, { kind: "camera_unavailable" });
    }
  };
  const preview = new PlayerModelPreviewProducer(manifest, (request) => native.framePlayerPreview(request));
  const placement = (state: RendererDiagnostics) => {
    const raw = native.scenePlacement();
    const value = sourceScenePlacement(state, raw, instanceTemplate);
    if (value?.blocks && raw?.blocks === false) {
      report("The renderer's legacy scenePlacement.blocks flag disagrees with its completed blocks@ scene. The shell normalizes only that flag from actual assembly diagnostics; this is not dynamic-minimap fidelity.");
    }
    return { raw, value };
  };
  return {
    resize(width, height) { native.resize(width, height); },
    async loadScene(id) {
      nativeCameraCurrent = false;
      invariant(sceneIds.has(id), `The actual renderer has no exported scene for ${id}. No fixture was selected as a fallback.`, "region_unavailable");
      await native.loadScene(id);
    },
    update(value: WorldView & RendererWorldExtensions) {
      // Extra validated fields (including dynamicObjects) survive the shared type boundary.
      preview.accept(value, () => native.update(value));
      world = value;
      nativeCameraCurrent = false;
      instanceTemplate = value.instanceLayout?.template ?? null;
      if (value.player.running === undefined || value.player.action === undefined) report("The authoritative movement/action observer is unavailable. The shell does not infer it from settings or nearby objects.");
    },
    camera(value) {
      invariant(value.near === 50 && value.unitsPerTurn === 16384, "Renderer camera must use its actual near50/16384-unit ABI.", "renderer");
      native.camera(value);
      nativeCameraMode = false;
      camera = { ...value, zoom: Math.trunc(value.zoom), far: Math.trunc(value.far) };
    },
    cameraSceneReady() { return cameraCall(() => native.cameraSceneReady()); },
    cameraSource() { return cameraCall(() => native.cameraSource()); },
    cameraScene() { return cameraCall(() => native.cameraScene()); },
    applyNativeCamera(delivery) {
      const value = cameraCall(() => native.applyNativeCamera(delivery));
      camera = { ...value };
      nativeCameraMode = true;
      nativeCameraCurrent = true;
      return value;
    },
    async frame(now) {
      if (disposed) return null;
      const mark = clock.mark();
      const frame = await native.frame(now);
      if (!frame || disposed) return null;
      const completed = await clock.completed(mark, frame);
      if (lastFrame === null || completed.sequence > lastFrame.sequence) lastFrame = completed;
      return completed;
    },
    pick(x, y) {
      const raw = native.pick(x, y);
      const resolved = canonicalPick(raw, world);
      if (raw !== null && resolved === null) report("Renderer pick has no valid tile and canonical WorldView identity; no interaction was dispatched.");
      return resolved;
    },
    supportsScene(id) { return sceneIds.has(id); },
    diagnostics() {
      const state = native.diagnostics();
      return { ...state, assets: residentRendererAssets(manifest, state), lastFrame };
    },
    framePlayerPreview(request) { return native.framePlayerPreview(request); },
    frameUiPreview(request, expected) { return preview.frame(request, expected); },
    clearPreviewMetadata() { preview.clear(); },
    playerFitReport() { return native.playerFitReport(); },
    playerPoseFits() { return native.playerPoseFits(); },
    observerV1() { return native.observerV1(); },
    playerRunning() { return native.playerRunning(); },
    unknownMotions() { return native.unknownMotions(); },
    unboundActions() { return native.unboundActions(); },
    minimapIconPlacements(x, y, scale, width, height) { return native.minimapIconPlacements(x, y, scale, width, height); },
    scenePlacement() { return placement(native.diagnostics()).value; },
    minimapSurface() { return native.minimapSurface(); },
    mapIconSprites() { return native.mapIconSprites(); },
    observe() {
      const state = native.diagnostics();
      const scene = placement(state);
      return {
        ready: state.sceneId !== null && state.deviceLostReason === null
          && (!nativeCameraMode || nativeCameraCurrent && cameraCall(() => native.cameraSceneReady())),
        sceneId: state.sceneId ?? "unloaded", assets: residentRendererAssets(manifest, state),
        scenePlacement: scene.value, nativeScenePlacement: scene.raw, loadedSquares: state.loadedSquares,
        actorObserver: { observerV1: native.observerV1(), running: native.playerRunning(),
          unknownMotions: native.unknownMotions(), unboundActions: native.unboundActions() },
        // The public adapter still discards raw entities_drawn; never substitute server counts.
        entities: {}, gpuTimestampPassScope: state.timestampsSupported ? "original integer fill compute pass" : null,
        settings: camera ? { backend: "webgpu", sourceManifestSha256: state.manifestSha256, brightness: manifest.brightness,
          near: 50, far: camera.far, zoom: camera.zoom, angleUnitsPerTurn: 16384,
          projection: nativeCameraMode ? "native-camera-integer-source-lane" : "renderer-native-full-hud-helper", fullHudProjectionMatched: false,
          normalCameraBrowserVerified: false,
          attachmentGapAccepted: false } : null,
      };
    },
    dispose() { if (!disposed) { disposed = true; preview.clear(); native.dispose(); clock.dispose(); } },
  };
}
