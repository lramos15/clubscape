import { createRenderer, sourceZoomForViewportHeight } from "../renderer/src/index.ts";
import type { ClubscapeRendererHandle, RenderAssetManifest, RendererDiagnostics } from "../renderer/src/index.ts";
import wasmUrl from "../renderer/pkg/clubscape_renderer_bg.wasm?url";
import type { RenderCamera, RendererConfig, RenderFrame, RendererHandle, WorldView } from "../shared/contracts.ts";
import type { RendererObservation } from "./benchmark.ts";
import { verifiedJson } from "./build.ts";
import { CanvasGpuClock } from "./gpu-clock.ts";
import { AppError, invariant } from "./errors.ts";
import { publicPath } from "./identity.ts";
import { canonicalPick } from "./picking.ts";

export { sourceZoomForViewportHeight };
export const RENDER_MANIFEST_SHA256 = "3fd1ec1953183de5537a2e7d239389c8dceed50c2e5d658dcc82c49113468845";

export interface ShellRenderer extends RendererHandle {
  observe(): RendererObservation;
  diagnostics(): RendererDiagnostics;
  supportsScene(id: string): boolean;
}

/** Exact adapter composition: real factory, real diagnostics, and the actual canvas queue clock. */
export async function createShellRenderer(canvas: HTMLCanvasElement, config: RendererConfig): Promise<ShellRenderer> {
  publicPath(config.manifestUrl);
  publicPath(`${config.assetBaseUrl.replace(/\/$/, "")}/manifest.json`);
  const { value } = await verifiedJson(config.manifestUrl, RENDER_MANIFEST_SHA256, 2 * 1024 * 1024, fetch.bind(globalThis));
  const manifest = value as RenderAssetManifest;
  const sceneIds = new Set(manifest.scenes.map((scene) => scene.name));
  const clock = new CanvasGpuClock(canvas);
  let native: ClubscapeRendererHandle;
  const reported = new Set<string>();
  const report = (message: string) => {
    if (reported.has(message)) return;
    reported.add(message);
    canvas.dispatchEvent(new CustomEvent("clubscape-render-diagnostic", { detail: message }));
  };
  try {
    native = await createRenderer(canvas, config, { wasmUrl, onDiagnostic: report });
  } catch (error) {
    clock.dispose();
    throw new AppError(`Actual WebGPU renderer initialization failed: ${String(error)}`, { kind: "device", recoverable: false });
  }
  let camera: RenderCamera | null = null;
  let world: WorldView | null = null;
  let inFlight = false;
  let disposed = false;
  let lastFrame: RenderFrame | null = null;
  return {
    resize(width, height) { native.resize(width, height); },
    async loadScene(id) {
      invariant(sceneIds.has(id), `The actual renderer has no exported scene for ${id}. No fixture was selected as a fallback.`, "region_unavailable");
      await native.loadScene(id);
    },
    update(value) { world = value; native.update(value); },
    camera(value) {
      invariant(value.near === 50 && value.unitsPerTurn === 16384, "Renderer camera must use its actual near50/16384-unit ABI.", "renderer");
      native.camera(value);
      camera = { ...value, zoom: Math.trunc(value.zoom), far: Math.trunc(value.far) };
    },
    async frame(now) {
      if (disposed || inFlight) return null;
      inFlight = true;
      const mark = clock.mark();
      try {
        const frame = await native.frame(now);
        if (!frame) return null;
        lastFrame = await clock.completed(mark, frame);
        return lastFrame;
      } finally { inFlight = false; }
    },
    pick(x, y) {
      const raw = native.pick(x, y);
      const resolved = canonicalPick(raw, world);
      if (raw?.kind === "entity" && resolved === null) report("Renderer object/actor pick hash has no unambiguous canonical WorldView identity; no interaction was dispatched.");
      return resolved;
    },
    supportsScene(id) { return sceneIds.has(id); },
    diagnostics() { return { ...native.diagnostics(), lastFrame }; },
    observe() {
      const state = native.diagnostics();
      const assets = new Map<string, { id: string; sha256: string; loaded: boolean }>();
      for (const asset of state.assets) assets.set(asset.id, { ...asset });
      return {
        ready: state.sceneId !== null && state.deviceLostReason === null,
        sceneId: state.sceneId ?? "unloaded", assets: [...assets.values()],
        // The initial public adapter discards raw entities_drawn; do not fabricate workload counts.
        entities: {}, gpuTimestampPassScope: state.timestampsSupported ? "original integer fill compute pass" : null,
        settings: camera ? { backend: "webgpu", sourceManifestSha256: state.manifestSha256, brightness: manifest.brightness,
          near: 50, far: camera.far, zoom: camera.zoom, angleUnitsPerTurn: 16384 } : null,
      };
    },
    dispose() { if (!disposed) { disposed = true; native.dispose(); clock.dispose(); } },
  };
}
