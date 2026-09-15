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
  /** World blocks: one 64x64 map square each (`square = x << 8 | y`), assembled around the player. */
  blocks?: Array<{
    square: number; file: string; sha256: string; models_file: string; models_sha256: string;
    origin_x: number; origin_y: number; size: number; file_gz?: string; models_file_gz?: string;
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
  /** Current scene base (world tiles) and the map squares loaded into the renderer. */
  sceneBase: { x: number; y: number } | null;
  loadedSquares: number[];
  timestampsSupported: boolean;
  deviceEpoch: string;
  manifestSha256: string;
  assets: LoadedAsset[];
  sceneId: string | null;
  renderedFrames: number;
  lastFrame: RenderFrame | null;
  deviceLostReason: string | null;
}

/** Approved model capture parameters (`drawFrustum(0, yaw, 0, 128, 0, cameraY, cameraZ)`, zoom 1024). */
export interface ModelFixtureRequest {
  /** Manifest model id (e.g. `models/object-1277-model-1570-lit.bin`) when `npc` is absent. */
  model?: string;
  npc?: { id: number; sequence: number; frame: number };
  yaw: number;
  cameraY: number;
  cameraZ: number;
}

export interface ClubscapeRendererHandle extends RendererHandle {
  diagnostics(): RendererDiagnostics;
  /** Developer-only replay of an approved model/animation capture on the canvas. */
  frameModelFixture(request: ModelFixtureRequest): Promise<RenderFrame>;
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
    const loadedModels = new Set<string>();

    let renderedFrames = 0;
    let lastFrame: RenderFrame | null = null;
    let inFlight = false;
    let disposed = false;
    let sceneId: string | null = null;
    // World-block streaming state (region scenes assembled around the player).
    let blockMode = false;
    let sceneBase: { x: number; y: number } | null = null;
    const loadedSquares = new Set<number>();
    const blockFetches = new Map<number, Promise<boolean>>();
    let assembling: Promise<void> | null = null;
    const blocksBySquare = new Map<number, NonNullable<RenderAssetManifest["blocks"]>[number]>();
    for (const block of manifest.blocks ?? []) blocksBySquare.set(block.square, block);
    const ensureBlock = (square: number): Promise<boolean> => {
      if (loadedSquares.has(square)) return Promise.resolve(true);
      const entry = blocksBySquare.get(square);
      if (!entry) return Promise.resolve(false); // not part of the exported world (empty in the original too)
      let pending = blockFetches.get(square);
      if (!pending) {
        pending = (async () => {
          const [blockBytes, packBytes] = await Promise.all([fetchAsset(entry.file_gz ?? entry.file), fetchAsset(entry.models_file_gz ?? entry.models_file)]);
          if (disposed) return false;
          renderer.load_block(square, blockBytes, packBytes);
          loadedSquares.add(square);
          return true;
        })().finally(() => blockFetches.delete(square));
        blockFetches.set(square, pending);
      }
      return pending;
    };
    /** Rebuilds the scene around `base` once every exported square it needs is loaded. */
    const assembleAround = (baseX: number, baseY: number): Promise<void> => {
      if (assembling) return assembling;
      assembling = (async () => {
        const squares = Array.from(WasmRenderer.squares_for_base(baseX, baseY));
        await Promise.all(squares.map((s) => ensureBlock(s)));
        if (disposed) return;
        const missing = Array.from(renderer.assemble_scene(baseX, baseY, performance.now()));
        const unexported = missing.filter((s) => blocksBySquare.has(s));
        if (unexported.length > 0) throw new Error(`blocks ${unexported.join(",")} were fetched but not loaded`);
        sceneBase = { x: baseX, y: baseY };
        sceneId = `blocks@${baseX},${baseY}`;
        // Keep only squares near the new scene resident.
        for (const square of Array.from(loadedSquares)) {
          if (!squares.includes(square)) { renderer.unload_block(square); loadedSquares.delete(square); }
        }
      })().finally(() => { assembling = null; });
      return assembling;
    };
    const REGION_ID = /^(?:region[.:]osrs[.:]|region[.:]|square[.:]|blocks?[.:])?(\d{4,5})$/;
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
        if (scene) {
          const [sceneBytes, packBytes] = await Promise.all([fetchAsset(scene.file_gz ?? scene.file), fetchAsset(scene.models_file_gz ?? scene.models_file)]);
          renderer.load_scene(id, sceneBytes, packBytes);
          sceneId = id;
          blockMode = false;
          sceneBase = null;
          return;
        }
        // Region scene: `region.osrs.12850` (content region ids), a bare map square id, or
        // `blocks@x,y` for an explicit chunk-aligned base. The world is assembled from blocks
        // around the square centre and follows the player afterwards (see update()).
        const explicit = /^blocks@(-?\d+),(-?\d+)$/.exec(id);
        const region = REGION_ID.exec(id);
        if (!explicit && !region) {
          throw new Error(`scene ${id} is not exported; available: ${manifest.scenes.map((s) => s.name).join(", ")} or region.osrs.<square>`);
        }
        let baseX: number;
        let baseY: number;
        if (explicit) {
          baseX = Number(explicit[1]);
          baseY = Number(explicit[2]);
        } else {
          const square = Number(region![1]);
          if (!blocksBySquare.has(square)) throw new Error(`map square ${square} is not part of the exported world`);
          const originX = (square >> 8) * 64;
          const originY = (square & 0xff) * 64;
          const base = WasmRenderer.base_for_tile(originX + 32, originY + 32);
          baseX = base[0]!;
          baseY = base[1]!;
        }
        blockMode = true;
        await assembleAround(baseX, baseY);
      },
      update(world: WorldView) {
        requireLive();
        renderer.update_world(JSON.stringify(world), performance.now());
        if (blockMode && !assembling) {
          const { x, y } = world.player.tile;
          if (renderer.needs_recenter(x, y, 16)) {
            const base = WasmRenderer.base_for_tile(x, y);
            void assembleAround(base[0]!, base[1]!).catch((error) => diagnostic(`scene recenter failed: ${String(error)}`));
          }
        }
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
      async frameModelFixture(request) {
        requireLive();
        if (inFlight) throw new Error("a frame is already in flight");
        inFlight = true;
        try {
          if (!request.npc) {
            if (!request.model) throw new Error("model fixture needs `model` or `npc`");
            if (!loadedModels.has(request.model)) {
              renderer.load_model(request.model, await fetchAsset(request.model));
              loadedModels.add(request.model);
            }
          }
          const record = await renderer.frame_model_fixture(
            request.model ?? "", request.npc?.id ?? -1, request.npc?.sequence ?? -1, request.npc?.frame ?? 0,
            Math.trunc(request.yaw), Math.trunc(request.cameraY), Math.trunc(request.cameraZ),
          );
          const frame: RenderFrame = {
            sequence: record.sequence, submittedAtMs: record.submitted_at_ms, completedAtMs: record.completed_at_ms,
            drawCalls: record.draw_calls, primitives: record.primitives, cpuEncodeMs: record.cpu_encode_ms,
            ...(record.gpu_duration_known ? { gpuDurationMs: record.gpu_duration_ms } : {}),
          };
          record.free();
          renderedFrames += 1;
          lastFrame = frame;
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
          sceneBase,
          loadedSquares: Array.from(loadedSquares).sort((a, b) => a - b),
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

/**
 * The original client's viewport zoom for a viewport height (662 at 1080 px, 883 at 1440 px,
 * 471 at 768 px). Pass it as `RenderCamera.zoom` unless reproducing another source zoom state;
 * the renderer never rescales on its own.
 */
export function sourceZoomForViewportHeight(height: number): number {
  const n = Math.trunc(height) - 334;
  const d = n < 0 ? 256 : n >= 100 ? 205 : Math.trunc(((205 - 256) * n) / 100) + 256;
  return Math.trunc((Math.trunc(height) * d) / 334);
}

/** Contract-typed alias for shells that only want the shared signature. */
export const createRendererContract: CreateRenderer = (canvas, config) => createRenderer(canvas, config);

/** Region scene id for a content region (`region.osrs.<square>`) or a map square number. */
export function regionSceneId(square: number): string {
  return `region.osrs.${square}`;
}

/** Exported scene ids in `assets/compiled/render/manifest.json` (fixture scenes of the approved pack). */
export const FIXTURE_SCENE_IDS = [
  "lumbridge-castle-plaza",
  "lumbridge-river-bridge",
  "tutorial-starting-house",
  "tutorial-survival-coast",
  "lumbridge-windmill-route",
] as const;
