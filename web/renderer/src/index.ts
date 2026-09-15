/**
 * ClubScape renderer adapter: implements `CreateRenderer` / `RendererHandle` from
 * `web/shared/contracts.ts` on top of the Rust/WASM WebGPU renderer (`crates/renderer`, feature
 * `web`, bindings generated into `web/renderer/pkg` by wasm-bindgen 0.2.128).
 *
 * The adapter owns asset fetching and hash verification (manifest-listed SHA-256), the WorldView
 * to renderer state hand-off, frame records (GPU completion, not requestAnimationFrame) and
 * picking. It never falls back to WebGL or images: every failure surfaces as a rejected promise or
 * thrown error carrying the original message.
 */
import type {
  CreateRenderer, RenderCamera, RenderFrame, RendererConfig, RendererHandle, ScenePick, WorldView,
} from "../../shared/contracts.ts";
import init, { WasmRenderer } from "../pkg/clubscape_renderer.js";

export interface RenderAssetManifest {
  schema_version: number;
  kind: "clubscape_render_assets";
  source_runtime: string;
  source_revision: number;
  approved_reference_pack_sha256: string;
  brightness: number;
  textures: number[];
  npcs: Array<{ npc_id: number; name: string; pack: string; sequences: Record<string, unknown> }>;
  scenes: Array<{
    name: string; file: string; sha256: string; models_file: string; models_sha256: string; base_x: number; base_y: number;
    /** Published gzip twins of the raw buffers (raw files are reproducible and not published). */
    file_gz?: string; models_file_gz?: string;
  }>;
  files: Record<string, { sha256: string; size_bytes: number; detail?: { encoding?: string; decompressed?: string; decompressed_sha256?: string } }>;
}

export interface LoadedAsset { id: string; sha256: string; loaded: boolean }

/** Extra, adapter-only options; the shared `RendererConfig` is not modified. */
export interface RendererAdapterOptions {
  /** URL of the wasm-bindgen output (`clubscape_renderer_bg.wasm`). Defaults to the sibling `pkg/` file. */
  wasmUrl?: string | URL;
  /** Hook receiving each GPU-completed frame record (for the benchmark probe). */
  onFrame?: (frame: RenderFrame) => void;
  /** Hook receiving non-fatal diagnostics (skipped entities, texture fallbacks). */
  onDiagnostic?: (message: string) => void;
}

/** Diagnostics the shell needs for the benchmark protocol (`RenderSnapshot`). */
export interface RendererDiagnostics {
  adapter: string;
  timestampsSupported: boolean;
  deviceEpoch: string;
  manifestSha256: string;
  assets: LoadedAsset[];
  sceneId: string | null;
  renderedFrames: number;
  lastFrame: RenderFrame | null;
  deviceLostReason: string | null;
}

export interface ClubscapeRendererHandle extends RendererHandle {
  diagnostics(): RendererDiagnostics;
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes as BufferSource);
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
}

function joinUrl(base: string, relative: string): string {
  const absoluteBase = new URL(base.endsWith("/") ? base : `${base}/`, document.baseURI);
  return new URL(relative, absoluteBase).toString();
}

function toInt(value: number, name: string): number {
  if (!Number.isFinite(value)) throw new Error(`camera.${name} must be finite, got ${value}`);
  return Math.trunc(value);
}

/**
 * Creates the renderer on `canvas`. Requires WebGPU; the promise rejects with the exact cause
 * otherwise. `config.sourcePackSha256` must equal the manifest's approved reference pack hash.
 */
export const createRenderer: (canvas: HTMLCanvasElement, config: RendererConfig, options?: RendererAdapterOptions) => Promise<ClubscapeRendererHandle> =
  async (canvas, config, options = {}) => {
    if (!("gpu" in navigator) || !navigator.gpu) throw new Error("WebGPU is not available in this browser (navigator.gpu missing)");
    const manifestResponse = await fetch(config.manifestUrl);
    if (!manifestResponse.ok) throw new Error(`manifest fetch failed: ${manifestResponse.status} ${config.manifestUrl}`);
    const manifestBytes = new Uint8Array(await manifestResponse.arrayBuffer());
    const manifestSha256 = await sha256Hex(manifestBytes);
    const manifest = JSON.parse(new TextDecoder().decode(manifestBytes)) as RenderAssetManifest;
    if (manifest.kind !== "clubscape_render_assets" || manifest.schema_version !== 1) {
      throw new Error(`unexpected render asset manifest kind/schema: ${manifest.kind}/${manifest.schema_version}`);
    }
    if (manifest.approved_reference_pack_sha256 !== config.sourcePackSha256) {
      throw new Error(`render assets were exported against source pack ${manifest.approved_reference_pack_sha256}, config expects ${config.sourcePackSha256}`);
    }
    const assets: LoadedAsset[] = [];
    const fetchAsset = async (id: string): Promise<Uint8Array> => {
      const entry = manifest.files[id];
      if (!entry) throw new Error(`asset ${id} is not listed in the render manifest`);
      const response = await fetch(joinUrl(config.assetBaseUrl, id));
      if (!response.ok) throw new Error(`asset fetch failed: ${response.status} ${id}`);
      let bytes = new Uint8Array(await response.arrayBuffer());
      const actual = await sha256Hex(bytes);
      if (actual !== entry.sha256) throw new Error(`asset ${id} hash mismatch: manifest ${entry.sha256}, fetched ${actual}`);
      if (entry.detail?.encoding === "gzip") {
        const stream = new Blob([bytes as BlobPart]).stream().pipeThrough(new DecompressionStream("gzip"));
        bytes = new Uint8Array(await new Response(stream).arrayBuffer());
        const inner = await sha256Hex(bytes);
        if (inner !== entry.detail.decompressed_sha256) throw new Error(`asset ${id} decompressed hash mismatch: manifest ${entry.detail.decompressed_sha256}, got ${inner}`);
        assets.push({ id: entry.detail.decompressed ?? id, sha256: inner, loaded: true });
      }
      assets.push({ id, sha256: actual, loaded: true });
      return bytes;
    };

    const wasmUrl = options.wasmUrl ?? new URL("../pkg/clubscape_renderer_bg.wasm", import.meta.url);
    await init({ module_or_path: wasmUrl });
    const palette = await fetchAsset("palette.bin");
    canvas.width = config.width;
    canvas.height = config.height;
    const renderer: WasmRenderer = await new WasmRenderer(canvas, config.width, config.height, palette);

    for (const textureId of manifest.textures) {
      renderer.add_texture(await fetchAsset(`textures/${textureId}.bin`));
    }
    for (const npc of manifest.npcs) {
      renderer.load_npc_pack(npc.npc_id, await fetchAsset(npc.pack));
    }

    let renderedFrames = 0;
    let lastFrame: RenderFrame | null = null;
    let inFlight = false;
    let disposed = false;
    let sceneId: string | null = null;
    const diagnostic = (message: string) => options.onDiagnostic?.(message);
    const requireLive = () => {
      if (disposed) throw new Error("renderer has been disposed");
      const lost = renderer.device_lost_reason();
      if (lost !== undefined) throw new Error(`WebGPU device lost: ${lost}`);
    };

    const handle: ClubscapeRendererHandle = {
      resize(width, height) {
        requireLive();
        const w = Math.max(1, Math.trunc(width));
        const h = Math.max(1, Math.trunc(height));
        canvas.width = w;
        canvas.height = h;
        renderer.resize(w, h);
      },
      async loadScene(id) {
        requireLive();
        const scene = manifest.scenes.find((entry) => entry.name === id);
        if (!scene) throw new Error(`scene ${id} is not exported; available: ${manifest.scenes.map((s) => s.name).join(", ")}`);
        const [sceneBytes, packBytes] = await Promise.all([fetchAsset(scene.file_gz ?? scene.file), fetchAsset(scene.models_file_gz ?? scene.models_file)]);
        renderer.load_scene(id, sceneBytes, packBytes);
        sceneId = id;
      },
      update(world: WorldView) {
        requireLive();
        renderer.update_world(JSON.stringify(world), performance.now());
      },
      camera(value: RenderCamera) {
        requireLive();
        if (value.unitsPerTurn !== 16384) throw new Error(`camera.unitsPerTurn must be 16384, got ${value.unitsPerTurn}`);
        renderer.set_camera(
          toInt(value.x, "x"), toInt(value.height, "height"), toInt(value.y, "y"),
          toInt(value.pitch, "pitch"), toInt(value.yaw, "yaw"), toInt(value.zoom, "zoom"), toInt(value.far, "far"),
        );
      },
      async frame(nowMs) {
        requireLive();
        if (inFlight || sceneId === null) return null;
        inFlight = true;
        try {
          const record = await renderer.frame(nowMs);
          const frame: RenderFrame = {
            sequence: record.sequence,
            submittedAtMs: record.submitted_at_ms,
            completedAtMs: record.completed_at_ms,
            drawCalls: record.draw_calls,
            primitives: record.primitives,
            cpuEncodeMs: record.cpu_encode_ms,
            ...(record.gpu_duration_known ? { gpuDurationMs: record.gpu_duration_ms } : {}),
          };
          if (record.skipped.length > 0) diagnostic(`entities skipped: ${record.skipped}`);
          if (record.texture_fallbacks > 0) diagnostic(`${record.texture_fallbacks} faces used the original missing-texture fallback`);
          record.free();
          renderedFrames += 1;
          lastFrame = frame;
          options.onFrame?.(frame);
          return frame;
        } finally {
          inFlight = false;
        }
      },
      pick(x, y) {
        requireLive();
        const json = renderer.pick(Math.trunc(x), Math.trunc(y));
        return json === undefined ? null : (JSON.parse(json) as ScenePick);
      },
      dispose() {
        if (disposed) return;
        disposed = true;
        renderer.free();
      },
      diagnostics() {
        return {
          adapter: renderer.adapter_info(),
          timestampsSupported: renderer.timestamps_supported(),
          deviceEpoch: String(renderer.device_epoch()),
          manifestSha256,
          assets: assets.slice(),
          sceneId,
          renderedFrames,
          lastFrame,
          deviceLostReason: renderer.device_lost_reason() ?? null,
        };
      },
    };
    return handle;
  };

/** Contract-typed alias for shells that only want the shared signature. */
export const createRendererContract: CreateRenderer = (canvas, config) => createRenderer(canvas, config);

/** Exported scene ids in `assets/compiled/render/manifest.json` (fixture scenes of the approved pack). */
export const FIXTURE_SCENE_IDS = [
  "lumbridge-castle-plaza",
  "lumbridge-river-bridge",
  "tutorial-starting-house",
  "tutorial-survival-coast",
  "lumbridge-windmill-route",
] as const;
