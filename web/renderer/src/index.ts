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
  ActorActionView, CreateRenderer, DynamicObjectView, RenderCamera, RenderFrame, RendererConfig, RendererHandle,
  ScenePick, WorldView,
} from "../../shared/contracts.ts";
import init, { WasmRenderer } from "../pkg/clubscape_renderer.js";
import { SourceSceneStream } from "./camera.ts";
import type { CameraSourceSample, NativeCameraRenderer } from "./camera.ts";
export type { CameraContext, CameraSourceSample, NativeCameraRenderer } from "./camera.ts";

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
  /** Minimap sidecars (`export.py --profile minimap`): per square wall configs + object map fields. */
  minimap_blocks?: Array<{ square: number; file: string; sha256: string; walls: number; object_definitions: number; map_icons?: number }>;
  /** Skeletal sequences (`export.py --profile anim`): player actions, NPC definition motions. */
  sequences?: Array<{ sequence_id: number; file: string; sha256: string; frame_count: number; frame_lengths_client_cycles: number[] }>;
  /** NPC definitions with their lit base model and original stand/walk/rotate/run sequence ids. */
  npc_definitions?: Array<{
    npc_id: number; name: string; base_model: string; size: number; width_scale: number; height_scale: number;
    sequences: { stand: number; walk: number; rotate_180: number; rotate_left: number; rotate_right: number; idle_rotate_left: number; idle_rotate_right: number; run: number };
    combat_sequences: number[];
  }>;
  /** Equippable M1 items: worn model (null for ammunition) and source params. */
  equipment_items?: Array<{ item_id: number; name: string; equip_model: string | null }>;
  /** Human reference body for label retargeting (the drawn body is NPC 2063 model 21547 at 75/128). */
  player_reference?: { model: string; classification: string };
  /**
   * Precomputed per-item per-pose gear fits (`export.py --profile pose-fits`): the shifts the
   * runtime contact solve would apply, so first display of a frame costs no solve. Frames over
   * the fit targets (0 in the published table) are counted here and reported per frame by
   * `playerPoseFits()`.
   */
  gear_pose_fits?: { file: string; schema_version: number; body_npc: number; items: number; sequences: number; item_frames: number; item_frames_over_target: number };
  /** `terrain/floors.bin`: every floor underlay/overlay definition, for the assembly-time terrain pass. */
  floor_definitions?: { file: string; sha256: string; underlays: number; overlays: number };
  /** Source `lc.bd` hand-item overrides of the required sequences (kit existence per value). */
  sequence_hand_overrides?: {
    rule: string;
    source: string;
    kit_count: number;
    values: Array<{ value: number; equipment_id: number; kind: "item" | "kit" | "none"; item_id?: number; kit_id?: number; kit_exists?: boolean; draws?: string }>;
  };
  /** Door/fire/state objects: per type+orientation lit models with optional baked frames. */
  dynamic_objects?: Array<{
    object_id: number; name: string; sequence: number;
    variants: Array<{ type: number; orientation: number; model: string; frames?: string[]; frame_lengths_client_cycles?: number[] }>;
  }>;
  /** Ground-item stack models per quantity threshold. */
  ground_items?: Array<{ item_id: number; name: string; price?: number; stackable?: boolean; variants: Array<{ min_quantity: number; model: string }> }>;
  /** Original interface model components (`export.py --profile widgets`), e.g. 679:73. */
  model_widgets?: Array<{
    id: number; group: number; child: number; parent: number; content_type: number; original_x: number; original_y: number;
    width: number; height: number; x_mode: number; y_mode: number; model_zoom: number; offset_x2: number; offset_y2: number;
    rotation_x: number; rotation_y: number; rotation_z: number; rasterizer_zoom: number; ortho: boolean;
  }>;
  files: Record<string, { sha256: string; size_bytes: number; detail?: { encoding?: string; decompressed?: string; decompressed_sha256?: string } }>;
}

/**
 * `WorldView.dynamicObjects` (`game.observer.v1`, shared `DynamicObjectView`): door states from
 * the protocol `WorldSnapshot.dynamic_objects`. Only `sourceId` — supplied by the shell's
 * validated definition-catalogue lookup — selects the source object; the renderer never parses
 * `objectId` strings, and an entry without `sourceId` is reported (`motion unknown: dynamic
 * object …`) and not drawn.
 */
export type RendererDynamicObject = DynamicObjectView;

/**
 * Optional shell extension mirroring the protocol `Event` with `kind === "animation"`: the
 * server's source action animation for an actor. `animationAsset` is
 * `asset.source.osrs.cache2695.sequence.<id>` (or a bare id). Each new `eventId` starts the
 * sequence on `actorId`; when it ends the actor returns to its movement/stand motion.
 */
export interface RendererAnimationEvent {
  kind: "animation";
  eventId: string;
  actorId: string;
  animationAsset: string;
}

/**
 * Renderer-consumed world inputs beyond the frozen `WorldView` fields. Motion identity is never
 * guessed. In precedence order the sources are:
 *
 * 1. `player.animation` / `entity.animation` (bare or catalog sequence ids);
 * 2. `game.observer.v1` — `running` / `movementTick` (movement actually executed this tick)
 *    and `action: ActorActionView` whose `animation` is played anchored to
 *    `cycleStartedAtTick` (600 ms per tick); the same `id` + cycle never restarts on polls or
 *    reconnects, and `animation: null` keeps the stance and reports the explicit action id as
 *    `motion unknown` — no nearby-object or default motion is selected;
 * 3. forwarded animation `events` (legacy shells);
 * 4. without observer fields, the original two-tiles-per-tick rule over `WorldView.tick` plus
 *    the `run` setting.
 */
export interface RendererWorldExtensions {
  dynamicObjects?: RendererDynamicObject[];
  events?: RendererAnimationEvent[];
  /**
   * The instance the player stands in, as the authoritative template identity and its
   * validated chunk mappings (backend `mechanics.instances` templates / `GenericInstanceChunkMapping`,
   * forwarded by the shell). While set, the block scene is assembled from the declared chunks
   * only — every other chunk stays unloaded (no terrain/scenery, black minimap) like the original
   * template loader — and `squares_needed` restricts block fetches to the declared source
   * squares. `null`/absent is the ordinary world. The renderer never infers a layout from an
   * instance id. `quarterTurns !== 0` is rejected explicitly (block exports carry lit placed
   * geometry that cannot be turned exactly).
   */
  instanceLayout?: RendererInstanceLayout | null;
}

/** One instance chunk mapping: destination chunk (absolute `tile >> 3`) showing a source chunk. */
export interface RendererChunkMapping {
  plane: number;
  chunkX: number;
  chunkY: number;
  sourcePlane: number;
  sourceChunkX: number;
  sourceChunkY: number;
  quarterTurns: number;
}

export interface RendererInstanceLayout {
  template: string;
  chunks: RendererChunkMapping[];
}

export type { ActorActionView };

/** Per-item gear fit on the penguin body (source units before the 75/128 draw scale). */
export interface PlayerFitReport {
  itemId: number;
  slot: string;
  humanLabel: number;
  penguinLabel: number;
  /** Deepest body vertex inside the item's oriented box after the contact solve (target ≤ 1). */
  penetration: number;
  /** Clearance between item and body surfaces, 0 when touching (target ≤ 2). */
  gap: number;
  /** Contact-solve translation from the retargeted design position (informational). */
  anchorShift: number;
  shiftDirection: [number, number, number];
  /** The box measure in the principal-axis frame alone (looser; shown so the box choice is visible). */
  pcaBoxPenetration: number;
  /** The same measure of the item on the human body it was designed for. */
  designPenetration: number;
  scale: number;
}

/** One per-pose gear fit (source units, unscaled body); see `RendererHandleExtensions.playerPoseFits`. */
export interface PlayerPoseFit {
  sequence: number;
  frame: number;
  itemId: number;
  slot: string;
  /** Length of the rigid translation applied this frame (body space, source units). */
  shift: number;
  direction: [number, number, number];
  /** Rotation about the posed grip point applied this frame (axis-angle, radians). */
  rotation: [number, number, number];
  precomputed: boolean;
  /** Carried-bind-box penetration after the fit (target ≤ 1 source unit). */
  penetration: number;
  /** Item↔body surface clearance after the fit (target ≤ 2): a surface metric only. */
  gap: number;
  /**
   * Attachment gap: how far the item's grip/contact point sits from where the posed anchor
   * bone (the item's slot part, retargeted) carries it (target ≤ 2). Distinguishes an item
   * that merely touches the body from one held/worn where the source attaches it.
   */
  attachmentGap: number;
  /** Clearance between the item and its anchor part alone (worn items must touch it). */
  anchorClearance: number;
  /** penetration ≤ 1 ∧ gap ≤ 2 ∧ attachmentGap ≤ 2. */
  meetsTargets: boolean;
}

/**
 * A source action the observer reported without a bound animation (`ActorActionView.animation:
 * null`): the exact identity the backend binding is owed for. Never turned into a guessed motion.
 */
export interface UnboundAction {
  actorId: string;
  id: string;
  activity: string;
  actionId: string | null;
  recipeId: string | null;
  styleId: string | null;
  spellId: string | null;
}

/** Model-only interface preview request; defaults reproduce interface 679 component 73. */
export interface PlayerPreviewRequest {
  /** Surface size in native pixels (the UI's `getUiPreviewBounds()` width/height). */
  width: number;
  height: number;
  /** Model component centre inside the surface; defaults to the exported component placement. */
  centerX?: number;
  centerY?: number;
  /** Original component content type (328 = character-design pitch/sway). */
  contentType?: number;
  rasterizerZoom?: number;
  modelZoom?: number;
  rotationX?: number;
  rotationY?: number;
  rotationZ?: number;
  offsetX?: number;
  offsetY?: number;
  /** Sequence/frame override (defaults: the player's idle motion on the renderer clock). */
  sequence?: number;
  frame?: number;
}

/** Scene placement for HUD helpers (the dynamic minimap draws over the source terrain raster). */
export interface ScenePlacement {
  baseX: number;
  baseY: number;
  sizeTiles: number;
  /** True when the scene is assembled from streamed world blocks (false for fixture scenes). */
  blocks: boolean;
}

/** A map-element icon position the original minimap widget draws (`bu.aa`); sprites are UI data. */
export interface MinimapIcon { x: number; y: number; plane: number; element: number }

/**
 * The source minimap raster of the current scene and plane: the original `client.bm(world,
 * 512x512, 4.0, plane, 0, 0, 48, 48)` sweep (terrain shapes/colours, wall/door/diagonal marks,
 * map-scene sprites) redrawn from the streamed blocks, door states and instance. Scene tile
 * (x, y) covers raster x `marginX + (x - baseX) * scale` .. +scale and y
 * `height - marginY - (y - baseY + 1) * scale` .. +scale; a player at (px, py) is centred at
 * `(marginX + (px - baseX) * scale + 2, height - marginY - (py - baseY) * scale - 2)`.
 */
export interface MinimapSurface {
  width: number;
  height: number;
  scale: number;
  marginX: number;
  marginY: number;
  baseX: number;
  baseY: number;
  plane: number;
  /** Increments whenever the raster was redrawn (scene, plane or door state changed). */
  revision: number;
  /** False when a placement lacked its exported config/definition; see `notes`. */
  complete: boolean;
  stats: { terrainTiles: number; wallMarks: number; diagonalMarks: number; mapScenes: number; unresolved: number };
  notes: string[];
  /**
   * Minimap icons on the drawn plane (the original `bu.aa` pass: floor decorations whose object
   * definition names a map element the original shows on the minimap), in world tiles. Drawn by
   * the HUD over `pixels` with `mapIconSprites()` and the original `client.zr`/`bo.as` rule,
   * which `placeMinimapIcon()` / `minimapIconPlacements()` compute exactly: `dx = ((x << 7) + 64
   * - playerFineX) * scale` truncated to **minimap pixels** (stock scale 1/32 → 4 px per tile),
   * skipped when `dx² + dy² > 6400` px² (80 px = 20 tiles), rotated by the minimap angle with the
   * 16384-step 16.16 sine/cosine tables, canvas top-left at `(W/2 + dx' - maxWidth/2, H/2 - dy'
   * - maxHeight/2)`; beyond 2500 px² (50 px) blitted through the widget's per-row mask
   * (`ym.bc`, no sprite offsets) instead of plainly (`aap.ro`, sprite offsets added). Never
   * baked into `pixels`.
   */
  icons: MinimapIcon[];
  /**
   * How `icons` compares with the original pass the block sidecars recorded for this plane:
   * 0 = identical sets; n = n icons differ (named in `notes`); null = sidecars without a record.
   */
  sourceIconMismatches: number | null;
  /** RGBA8 (alpha 255 everywhere, as the native capture wrote it). */
  pixels: ImageData;
  /** 1 where the original sweep drew map data, 0 where no tile exists (the source fill value). */
  mask: Uint8Array;
}

/** One original map-element minimap sprite (`minimap/mapicons.bin`, `ps.as(false)`). */
export interface MapIconSprite {
  element: number;
  width: number;
  height: number;
  offsetX: number;
  offsetY: number;
  maxWidth: number;
  maxHeight: number;
  category: number;
  /** RGBA8; alpha 0 exactly where the original blit skips the pixel (value 0), 255 elsewhere. */
  pixels: ImageData;
}

/**
 * Exact `client.zr` → `bo.as` placement of one minimap marker, in minimap pixels relative to the
 * widget origin. `x/y` is the sprite canvas top-left (`W/2 + dx' - maxWidth/2`, `H/2 - dy' -
 * maxHeight/2`); `drawX/drawY` is where the stored `width × height` sub-image lands: plain blit
 * (`aap.ro`, within 50 px) adds the sprite's `offsetX/offsetY`, the mask-clipped blit (`ym.bc`,
 * beyond 50 px) does not and limits each row to the widget sprite's visible span. Value-0
 * sprite pixels (alpha 0 in `MapIconSprite.pixels`) are skipped in both.
 */
export interface MinimapIconPlacement {
  x: number;
  y: number;
  drawX: number;
  drawY: number;
  clipped: boolean;
  /** Scaled, unrotated offset from the player in minimap pixels. */
  dx: number;
  dy: number;
}

/** `minimapIconPlacements()`: every in-range marker of the current surface placed for a widget. */
export interface MinimapIconPlacements {
  /** Minimap angle used (the camera yaw, 16384 units per turn — `client.jv`). */
  minimapAngle: number;
  scale: number;
  /** Markers whose element has no loaded sprite (omitted from `icons`). */
  missingSprites: number;
  icons: Array<MinimapIconPlacement & { element: number; tileX: number; tileY: number }>;
}

/** Stock minimap scale (`client.fa` → `zo(..., 0.03125F)`): 128 fine units = 4 px. */
export const MINIMAP_STOCK_SCALE = 0.03125;

export interface LoadedAsset { id: string; sha256: string; loaded: boolean }

/** `RenderFrame` plus the adapter's CPU breakdown of `cpuEncodeMs` (ms on the main thread). */
export interface ClubscapeRenderFrame extends RenderFrame {
  cpuBreakdownMs?: {
    /** Scene traversal + projection (`build_frame`). */
    build: number;
    /** Triangle stream → GPU layout + bin lists (`pack_frame`). */
    pack: number;
    /** Buffer uploads + compute encode/submit. */
    upload: number;
    /** Canvas texture acquire, blit, present. */
    present: number;
  };
}

/** Extra, adapter-only options; the shared `RendererConfig` is not modified. */
export interface RendererAdapterOptions {
  /** URL of the wasm-bindgen output (`clubscape_renderer_bg.wasm`). Defaults to the sibling `pkg/` file. */
  wasmUrl?: string | URL;
  /** Hook receiving each GPU-completed frame record (for the benchmark probe). */
  onFrame?: (frame: RenderFrame) => void;
  /** Hook receiving non-fatal diagnostics (skipped entities, texture fallbacks). */
  onDiagnostic?: (message: string) => void;
  /**
   * Frames allowed in flight (default 2): the next frame's CPU build overlaps the previous
   * frame's GPU completion. 1 serializes build → completion like a single-buffered loop.
   */
  maxFramesInFlight?: number;
  /**
   * Developer fixtures only: derive action motions from `activity` and adjacent scenery when no
   * source animation is supplied. Off by default — not final M1 logic; production shells leave
   * motion identity to the server's `animation` fields and animation events.
   */
  developerMotionFallback?: boolean;
}

/** Diagnostics the shell needs for the benchmark protocol (`RenderSnapshot`). */
export interface RendererDiagnostics {
  adapter: string;
  /** Current scene base (world tiles) and the map squares loaded into the renderer. */
  sceneBase: { x: number; y: number } | null;
  loadedSquares: number[];
  /**
   * The current block scene's terrain pass, or `null` when no block scene is loaded.
   * Missing raw terrain or floor definitions are assembly errors, never approximate scenes.
   */
  terrainRebuilt: { paints: number; tileModels: number; missingOverlays: number; missingUnderlays: number } | null;
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

export interface ClubscapeRendererHandle extends RendererHandle, NativeCameraRenderer {
  diagnostics(): RendererDiagnostics;
  /** Developer-only replay of an approved model/animation capture on the canvas. */
  frameModelFixture(request: ModelFixtureRequest): Promise<RenderFrame>;
  /**
   * Renders the model-only player preview (current body + gear) through the original interface
   * model projection into `ImageData` of exactly `width` × `height` with coverage alpha, after
   * the GPU completed the work. Resolves `null` when no player body is loaded. Intended for the
   * UI's `setUiPreview()`; never a reference capture.
   */
  framePlayerPreview(request: PlayerPreviewRequest): Promise<ImageData | null>;
  /**
   * Top drawn plane (`br`). `null` = the stock live rule (all planes unless a roof-flagged tile of
   * the player's plane lies on the camera→player line at pitch < 2480). The approved fixture
   * captures were taken with plane 0; `loadScene()` of a fixture scene pins 0 and region scenes
   * restore the stock rule, so shells only call this to deviate deliberately.
   */
  setTopPlane(limit: number | null): void;
  /** Instanced map flag (`dz.ag`): the stock top-plane rule then draws up to the player's plane. */
  setInstancedMap(instanced: boolean): void;
  /**
   * The original "hide roofs" client preference (`cy.as`, read first by the stock `cz.ch`
   * selector): the top drawn plane is the player's plane. Off by default; the UI's settings own it.
   */
  setHideRoofs(hidden: boolean): void;
  /**
   * Developer fixture control only (not a gameplay state): draw no body for the local player, as
   * in the controlled original dynamic-layer references, which were rendered without one. The
   * player's tile still drives the plane, roof rule and minimap.
   */
  setHideLocalPlayerBody(hidden: boolean): void;
  /**
   * Developer fixture control only: replay an original capture's recorded animated-scenery
   * controller state (`dy.ac` frame and cycle within the frame at scene start) for the animated
   * instances of `objectId` on a world tile; returns how many instances were set.
   */
  setSceneryPhase(plane: number, x: number, y: number, objectId: number, frame: number, cycle: number): number;
  /**
   * Developer fixture control only: freeze the scenery animation clock at `cycles` client cycles
   * since scene start (`null` = real time), to render at exactly the cycle a capture was drawn at.
   */
  setSceneryClock(cycles: number | null): void;
  /** Original roof-removal mode bits (1 player, 2 hovered, 4 destination, 8 camera line); 0 = stock. */
  setRoofMode(mode: number): void;
  /** Hovered/destination tiles consulted by roof modes 2 and 4. */
  setRoofContext(hovered: { x: number; y: number } | null, destination: { x: number; y: number } | null): void;
  /** Current gear fit report (empty until a body and gear are assembled). */
  playerFitReport(): PlayerFitReport[];
  /**
   * Per-pose gear fits of every player frame drawn or previewed since the gear last changed:
   * the rigid per-frame rotation (about the grip) + shift each drawn item received against the
   * posed body (from the precomputed table or a live solve) and the penetration / surface gap /
   * attachment gap it left. Items a sequence hides (`lc.bd` hand overrides) have no entry.
   * `meetsTargets: false` entries are the exact remaining fit failures (targets: penetration
   * ≤ 1, gap ≤ 2, attachmentGap ≤ 2 source units).
   */
  playerPoseFits(): PlayerPoseFit[];
  /**
   * Source actions the last `update()` reported with `animation: null` (exact ids per actor),
   * awaiting the backend's animation binding. Also reported through `unknownMotions()`.
   */
  unboundActions(): UnboundAction[];
  /** Current scene placement (null without a scene). */
  scenePlacement(): ScenePlacement | null;
  /**
   * The source minimap of the current scene on the player's plane (see `MinimapSurface`), for
   * the UI's minimap widget. Throws when the scene, the map-scene asset or a square's minimap
   * sidecar is missing — never a blank or approximate map. Cached until the state changes.
   */
  minimapSurface(): MinimapSurface;
  /**
   * The original map-element minimap sprites (`minimap/mapicons.bin`), keyed by element for
   * `MinimapSurface.icons`. Empty until the asset is loaded (fetched with the first world block).
   */
  mapIconSprites(): Map<number, MapIconSprite>;
  /**
   * Exact `client.zr`/`bo.as` placement of one marker at world tile `(tileX, tileY)` for a player
   * standing on world tile `(playerTileX, playerTileY)` (actors are drawn at tile centres, so the
   * player's fine position is `tile * 128 + 64`), a minimap zoom `scale` (`MINIMAP_STOCK_SCALE`
   * or RuneLite `zoom / 128`), the minimap angle (camera yaw, 16384 units), the widget sprite
   * size and the marker's sprite. `null` when the marker is farther than 80 minimap pixels.
   */
  placeMinimapIcon(
    tileX: number, tileY: number, playerTileX: number, playerTileY: number, scale: number, minimapAngle: number,
    widgetWidth: number, widgetHeight: number, sprite: Pick<MapIconSprite, "maxWidth" | "maxHeight" | "offsetX" | "offsetY">,
  ): MinimapIconPlacement | null;
  /**
   * Every in-range marker of the current `minimapSurface()` placed for a widget, with the loaded
   * sprites and the current camera yaw as the minimap angle. Throws like `minimapSurface()` when
   * the scene or map data is missing, or when the sprites are not loaded.
   */
  minimapIconPlacements(playerTileX: number, playerTileY: number, scale: number, widgetWidth: number, widgetHeight: number): MinimapIconPlacements;
  /**
   * Whether the player is running: `PlayerView.running` (`game.observer.v1`, the movement
   * actually executed this tick) when present, else the two-tiles-per-server-tick rule (or the
   * `run` setting without ticks).
   */
  playerRunning(): boolean;
  /**
   * Whether the last `update()` carried `game.observer.v1` fields (`running`, `movementTick`,
   * `action`). False means an older observer: movement is inferred from ticks and actions
   * play only from `animation` / forwarded events.
   */
  observerV1(): boolean;
  /**
   * Actors whose reported state implies an action but whose source motion the last `update()`
   * did not supply (also reported through `onDiagnostic` as `motion unknown: …`).
   */
  unknownMotions(): string[];
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
    const assetLoads = new AbortController();
    const manifestResponse = await fetch(config.manifestUrl, { signal: assetLoads.signal });
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
      const response = await fetch(joinUrl(config.assetBaseUrl, id), { signal: assetLoads.signal });
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
    // Skeletal animation, NPC definitions, the approved player body and its gear.
    for (const sequence of manifest.sequences ?? []) {
      renderer.load_sequence(await fetchAsset(sequence.file));
    }
    for (const definition of manifest.npc_definitions ?? []) {
      renderer.load_npc_definition(JSON.stringify(definition), await fetchAsset(definition.base_model));
    }
    const penguin = manifest.npc_definitions?.find((d) => d.npc_id === 2063);
    if (penguin && manifest.player_reference) {
      renderer.load_player_body(
        await fetchAsset(penguin.base_model), penguin.width_scale, penguin.height_scale,
        Int32Array.from([penguin.sequences.stand, penguin.sequences.walk]), await fetchAsset(manifest.player_reference.model),
      );
    }
    for (const item of manifest.equipment_items ?? []) {
      if (item.equip_model) renderer.load_equip_model(item.item_id, await fetchAsset(item.equip_model));
    }
    if (manifest.sequence_hand_overrides) {
      renderer.set_hand_override_kits(
        new Int32Array(
          manifest.sequence_hand_overrides.values
            .filter((v) => v.kind === "kit" && v.kit_exists === true && typeof v.kit_id === "number")
            .map((v) => v.kit_id as number),
        ),
      );
    }
    if (manifest.gear_pose_fits && penguin) {
      const table = new TextDecoder().decode(await fetchAsset(manifest.gear_pose_fits.file));
      const entries = renderer.load_pose_fit_table(table, penguin.npc_id);
      options.onDiagnostic?.(
        `gear pose fits: ${entries} item×sequence entries loaded; ${manifest.gear_pose_fits.item_frames_over_target} of ${manifest.gear_pose_fits.item_frames} item-frames over the fit targets`,
      );
    } else if (penguin) {
      options.onDiagnostic?.("gear pose fits: no precomputed table in the manifest; player frames solve their fit live on first display");
    }
    // Live layers: doors/fires/state objects and ground-item stacks.
    for (const object of manifest.dynamic_objects ?? []) {
      for (const variant of object.variants) {
        const frames = await Promise.all((variant.frames ?? []).map((file) => fetchAsset(file)));
        renderer.load_dynamic_object(
          object.object_id, variant.type, variant.orientation, await fetchAsset(variant.model),
          frames, Int32Array.from(variant.frame_lengths_client_cycles ?? []),
        );
      }
    }
    for (const item of manifest.ground_items ?? []) {
      if (typeof item.price === "number" && typeof item.stackable === "boolean") {
        renderer.register_ground_item_definition(item.item_id, item.price, item.stackable);
      } else {
        options.onDiagnostic?.(`ground item ${item.item_id}: manifest lacks price/stackable (pile order will use value 0)`);
      }
      for (const variant of item.variants) {
        renderer.load_ground_item(item.item_id, variant.min_quantity, await fetchAsset(variant.model));
      }
    }
    const previewWidget = manifest.model_widgets?.find((w) => w.id === 44499017);
    if (options.developerMotionFallback) renderer.set_motion_fallback(true);
    const loadedModels = new Set<string>();

    let renderedFrames = 0;
    let lastFrame: RenderFrame | null = null;
    // Frames in flight: one may be awaiting GPU completion while the next is being built, so
    // CPU scene traversal overlaps GPU execution. Each promise still resolves only on genuine
    // completion of its own submission (the queue executes in order).
    let inFlight = 0;
    const MAX_IN_FLIGHT = options.maxFramesInFlight ?? 2;
    let exclusive = false;
    let disposed = false;
    let sceneId: string | null = null;
    // World-block streaming state (region scenes assembled around the player).
    let blockMode = false;
    let sceneBase: { x: number; y: number } | null = null;
    const loadedSquares = new Set<number>();
    const blockFetches = new Map<number, Promise<boolean>>();
    let sceneLoadEpoch = 0;
    let sceneLoadPending = false;
    let layoutKey = "null";
    const blocksBySquare = new Map<number, NonNullable<RenderAssetManifest["blocks"]>[number]>();
    for (const block of manifest.blocks ?? []) blocksBySquare.set(block.square, block);
    const minimapBySquare = new Map<number, NonNullable<RenderAssetManifest["minimap_blocks"]>[number]>();
    for (const entry of manifest.minimap_blocks ?? []) minimapBySquare.set(entry.square, entry);
    let mapScenes: Promise<void> | null = null;
    /**
     * The map-scene sprites/shape masks, the map-element icon sprites and the floor definitions
     * (for the assembly-time terrain pass), fetched once with the first world block.
     */
    const ensureMapScenes = (): Promise<void> => {
      mapScenes ??= (async () => {
        if (!manifest.floor_definitions) {
          throw new Error("terrain: manifest has no required floor_definitions for block scenes");
        }
        const floors = await fetchAsset(manifest.floor_definitions.file);
        if (!disposed) {
          const [underlays, overlays] = renderer.load_floor_defs(floors);
          diagnostic(`terrain: ${underlays} underlay / ${overlays} overlay definitions loaded; block scenes run the original terrain pass at their own base`);
        }
        if (!manifest.files["minimap/mapscenes.bin"]) return;
        const bytes = await fetchAsset("minimap/mapscenes.bin");
        if (!disposed) renderer.load_map_scenes(bytes);
        if (manifest.files["minimap/mapicons.bin"]) {
          const icons = await fetchAsset("minimap/mapicons.bin");
          if (!disposed) renderer.load_map_icons(icons);
        } else {
          diagnostic("minimap: manifest lists no minimap/mapicons.bin; the HUD has no original icon sprites to draw");
        }
      })();
      return mapScenes;
    };
    let mapIconCache: Map<number, MapIconSprite> | null = null;
    const ensureBlock = (square: number): Promise<boolean> => {
      if (loadedSquares.has(square)) return Promise.resolve(true);
      const entry = blocksBySquare.get(square);
      if (!entry) return Promise.resolve(false); // not part of the exported world (empty in the original too)
      let pending = blockFetches.get(square);
      if (!pending) {
        pending = (async () => {
          const minimap = minimapBySquare.get(square);
          const [blockBytes, packBytes, sidecar] = await Promise.all([
            fetchAsset(entry.file_gz ?? entry.file),
            fetchAsset(entry.models_file_gz ?? entry.models_file),
            minimap ? fetchAsset(minimap.file) : Promise.resolve(null),
            ensureMapScenes(),
          ]);
          if (disposed) return false;
          renderer.load_block(square, blockBytes, packBytes);
          if (sidecar) renderer.load_minimap_block(square, sidecar);
          loadedSquares.add(square);
          return true;
        })().finally(() => blockFetches.delete(square));
        blockFetches.set(square, pending);
      }
      return pending;
    };
    const sceneStream = new SourceSceneStream(
      (target: { baseX: number; baseY: number; layout: string }) => JSON.stringify(target),
      async ({ baseX, baseY }) => {
        // Inside an instance only the declared source squares are fetched and assembled.
        const squares = Array.from(renderer.squares_needed(baseX, baseY));
        await Promise.all(squares.map((s) => ensureBlock(s)));
        return squares;
      },
      ({ baseX, baseY }, squares) => {
        if (disposed) return;
        const missing = Array.from(renderer.assemble_scene(baseX, baseY, performance.now()));
        const unexported = missing.filter((s) => blocksBySquare.has(s));
        if (unexported.length > 0) throw new Error(`blocks ${unexported.join(",")} were fetched but not loaded`);
        sceneBase = { x: baseX, y: baseY };
        // The core's own id: `blocks@x,y` or `blocks@x,y#<instance template>` inside an instance.
        sceneId = renderer.scene_id() ?? `blocks@${baseX},${baseY}`;
        // Keep only squares near the new scene resident.
        for (const square of Array.from(loadedSquares)) {
          if (!squares.includes(square)) { renderer.unload_block(square); loadedSquares.delete(square); }
        }
      },
    );
    const assembleAround = (baseX: number, baseY: number): Promise<void> =>
      sceneStream.request({ baseX, baseY, layout: layoutKey });
    let lastPlayerTile: { x: number; y: number } | null = null;
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
        const epoch = ++sceneLoadEpoch;
        const scene = manifest.scenes.find((entry) => entry.name === id);
        if (scene) {
          sceneStream.cancel();
          blockMode = false;
          sceneLoadPending = true;
          try {
            const [sceneBytes, packBytes] = await Promise.all([fetchAsset(scene.file_gz ?? scene.file), fetchAsset(scene.models_file_gz ?? scene.models_file)]);
            if (disposed || epoch !== sceneLoadEpoch) throw new Error("Source scene load was superseded.");
            renderer.load_scene(id, sceneBytes, packBytes);
            // Fixture scenes reproduce the approved captures: original `dh` plane argument 0.
            renderer.set_top_plane_override(0);
            sceneId = id;
            sceneBase = null;
          } finally {
            if (epoch === sceneLoadEpoch) sceneLoadPending = false;
          }
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
          const base = WasmRenderer.base_for_tile(lastPlayerTile?.x ?? originX + 32, lastPlayerTile?.y ?? originY + 32);
          baseX = base[0]!;
          baseY = base[1]!;
        }
        sceneLoadPending = false;
        blockMode = true;
        renderer.set_top_plane_override(undefined);
        await assembleAround(baseX, baseY);
        if (disposed || epoch !== sceneLoadEpoch) throw new Error("Source scene load was superseded.");
      },
      update(world: WorldView & RendererWorldExtensions) {
        requireLive();
        renderer.update_world(JSON.stringify(world), performance.now());
        layoutKey = JSON.stringify(world.instanceLayout ?? null);
        lastPlayerTile = { x: world.player.tile.x, y: world.player.tile.y };
        if (blockMode) {
          const { x, y } = world.player.tile;
          // A changed instance layout rebuilds the scene from the declared chunks (or back to
          // the ordinary world); otherwise the original 16-tile edge rule recentres it.
          if (renderer.instance_layout_changed() || renderer.needs_recenter(x, y, 16)) {
            const base = WasmRenderer.base_for_tile(x, y);
            void assembleAround(base[0]!, base[1]!).catch((error) => diagnostic(`scene assembly failed: ${String(error)}`));
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
      cameraSceneReady() {
        requireLive();
        if (sceneLoadPending || sceneStream.busy) return false;
        sceneStream.requireReady();
        return blockMode && sceneId !== null;
      },
      cameraSource() {
        requireLive();
        sceneStream.requireReady();
        if (!blockMode || sceneLoadPending) throw new Error("Normal camera requires a completed real source-block scene, not a diagnostic fixture.");
        return JSON.parse(renderer.camera_source()) as CameraSourceSample;
      },
      cameraScene() {
        requireLive();
        sceneStream.requireReady();
        if (!blockMode || sceneLoadPending) throw new Error("Normal camera terrain is not ready.");
        return renderer.camera_scene();
      },
      applyNativeCamera(delivery) {
        requireLive();
        sceneStream.requireReady();
        if (!blockMode || sceneLoadPending) throw new Error("Native camera delivery cannot target a stale/diagnostic scene.");
        return JSON.parse(renderer.apply_native_camera(delivery)) as RenderCamera;
      },
      async frame(nowMs) {
        requireLive();
        if (sceneStream.busy) return null;
        sceneStream.requireReady();
        if (exclusive || inFlight >= MAX_IN_FLIGHT || sceneId === null || sceneLoadPending) return null;
        inFlight += 1;
        try {
          const record = await renderer.frame(nowMs);
          const frame: ClubscapeRenderFrame = {
            sequence: record.sequence,
            submittedAtMs: record.submitted_at_ms,
            completedAtMs: record.completed_at_ms,
            drawCalls: record.draw_calls,
            primitives: record.primitives,
            cpuEncodeMs: record.cpu_encode_ms,
            cpuBreakdownMs: { build: record.cpu_build_ms, pack: record.cpu_pack_ms, upload: record.cpu_upload_ms, present: record.cpu_present_ms },
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
          inFlight -= 1;
        }
      },
      async frameModelFixture(request) {
        requireLive();
        if (inFlight > 0 || exclusive) throw new Error("a frame is already in flight");
        exclusive = true;
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
          exclusive = false;
        }
      },
      pick(x, y) {
        requireLive();
        const json = renderer.pick(Math.trunc(x), Math.trunc(y));
        return json === undefined ? null : (JSON.parse(json) as ScenePick);
      },
      async framePlayerPreview(request) {
        requireLive();
        const width = Math.trunc(request.width);
        const height = Math.trunc(request.height);
        if (!(width > 0 && height > 0)) throw new Error(`preview size ${request.width}x${request.height} is empty`);
        // The exported component sits at its original offset inside the parent layer; centre it
        // the way the original layout does (xMode 1 centre, yMode 2 bottom-anchored originalY).
        const defaults = previewWidget
          ? {
            centerX: Math.trunc((width - previewWidget.width) / 2) + Math.trunc(previewWidget.width / 2),
            centerY: height - previewWidget.original_y - previewWidget.height + Math.trunc(previewWidget.height / 2),
            contentType: previewWidget.content_type, rasterizerZoom: previewWidget.rasterizer_zoom, modelZoom: previewWidget.model_zoom,
            rotationX: previewWidget.rotation_x, rotationY: previewWidget.rotation_y, rotationZ: previewWidget.rotation_z,
            offsetX: previewWidget.offset_x2, offsetY: previewWidget.offset_y2,
          }
          : {};
        const options = { ...defaults, ...request, width, height };
        const pixels = (await renderer.frame_player_preview(JSON.stringify(options), performance.now())) as Uint8ClampedArray | undefined;
        if (pixels === undefined) return null;
        if (pixels.length !== width * height * 4) throw new Error(`preview readback returned ${pixels.length} bytes for ${width}x${height}`);
        const image = new ImageData(width, height);
        image.data.set(pixels);
        return image;
      },
      setTopPlane(limit) {
        requireLive();
        renderer.set_top_plane_override(limit === null ? undefined : Math.trunc(limit));
      },
      setInstancedMap(instanced) {
        requireLive();
        renderer.set_instanced_map(Boolean(instanced));
      },
      setHideRoofs(hidden) {
        requireLive();
        renderer.set_hide_roofs(hidden);
      },
      setHideLocalPlayerBody(hidden) {
        requireLive();
        renderer.set_hide_local_player_body(hidden);
      },
      setSceneryPhase(plane, x, y, objectId, frame, cycle) {
        requireLive();
        return renderer.set_scenery_phase(plane, x, y, objectId, frame, cycle);
      },
      setSceneryClock(cycles) {
        requireLive();
        renderer.set_scenery_clock_override(cycles === null ? -1 : Math.trunc(cycles));
      },
      setRoofMode(mode) {
        requireLive();
        renderer.set_roof_mode(Math.trunc(mode));
      },
      setRoofContext(hovered, destination) {
        requireLive();
        renderer.set_roof_context(
          hovered ? Math.trunc(hovered.x) : undefined, hovered ? Math.trunc(hovered.y) : undefined,
          destination ? Math.trunc(destination.x) : undefined, destination ? Math.trunc(destination.y) : undefined,
        );
      },
      playerFitReport() {
        requireLive();
        return JSON.parse(renderer.player_fit_report()) as PlayerFitReport[];
      },
      playerPoseFits() {
        requireLive();
        return JSON.parse(renderer.player_pose_fits()) as PlayerPoseFit[];
      },
      scenePlacement() {
        requireLive();
        const json = renderer.scene_placement();
        return json === undefined ? null : (JSON.parse(json) as ScenePlacement);
      },
      minimapSurface() {
        requireLive();
        const meta = JSON.parse(renderer.minimap_surface()) as Omit<MinimapSurface, "pixels" | "mask">;
        const rgba = renderer.minimap_pixels();
        const mask = renderer.minimap_mask();
        const image = new ImageData(meta.width, meta.height);
        image.data.set(rgba);
        return { ...meta, pixels: image, mask };
      },
      mapIconSprites() {
        requireLive();
        if (mapIconCache) return mapIconCache;
        const meta = JSON.parse(renderer.map_icon_sprites()) as Array<Omit<MapIconSprite, "pixels">>;
        if (meta.length === 0) return new Map();
        const pixels = renderer.map_icon_pixels();
        const out = new Map<number, MapIconSprite>();
        let cursor = 0;
        for (const sprite of meta) {
          const count = sprite.width * sprite.height * 4;
          const image = new ImageData(Math.max(1, sprite.width), Math.max(1, sprite.height));
          if (count > 0) image.data.set(pixels.subarray(cursor, cursor + count));
          cursor += count;
          out.set(sprite.element, { ...sprite, pixels: image });
        }
        mapIconCache = out;
        return out;
      },
      placeMinimapIcon(tileX, tileY, playerTileX, playerTileY, scale, minimapAngle, widgetWidth, widgetHeight, sprite) {
        if (disposed) return null;
        const json = renderer.minimap_icon_placement(
          tileX, tileY, playerTileX * 128 + 64, playerTileY * 128 + 64, scale, minimapAngle, widgetWidth, widgetHeight,
          sprite.maxWidth, sprite.maxHeight, sprite.offsetX, sprite.offsetY,
        );
        return json === undefined ? null : (JSON.parse(json) as MinimapIconPlacement);
      },
      minimapIconPlacements(playerTileX, playerTileY, scale, widgetWidth, widgetHeight) {
        if (disposed) throw new Error("renderer disposed");
        return JSON.parse(renderer.minimap_icon_placements(playerTileX, playerTileY, scale, widgetWidth, widgetHeight)) as MinimapIconPlacements;
      },
      playerRunning() {
        requireLive();
        return renderer.player_running();
      },
      observerV1() {
        requireLive();
        return renderer.observer_v1();
      },
      unboundActions() {
        requireLive();
        return JSON.parse(renderer.unbound_actions()) as UnboundAction[];
      },
      unknownMotions() {
        requireLive();
        return JSON.parse(renderer.unknown_motions()) as string[];
      },
      dispose() {
        if (disposed) return;
        disposed = true;
        assetLoads.abort();
        sceneLoadEpoch++;
        sceneStream.cancel();
        renderer.free();
      },
      diagnostics() {
        return {
          adapter: renderer.adapter_info(),
          sceneBase,
          loadedSquares: Array.from(loadedSquares).sort((a, b) => a - b),
          terrainRebuilt: (() => { const t = renderer.terrain_rebuilt(); return t === undefined ? null : JSON.parse(t); })(),
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
 * The original client's viewport-only zoom (`client.oh` with the stock parameters fy 256 /
 * fg 205) for a viewport height: 662 at 1080 px, 883 at 1440 px, 471 at 768 px. This is the
 * projection of the frozen viewport-only scene fixtures (`assets/reference/osrs240/*`); use
 * `fullHudZoomForViewport` for the composed Resizable-Classic HUD.
 */
export function sourceZoomForViewportHeight(height: number): number {
  const n = Math.trunc(height) - 334;
  const d = n < 0 ? 256 : n >= 100 ? 205 : Math.trunc(((205 - 256) * n) / 100) + 256;
  return Math.trunc((Math.trunc(height) * d) / 334);
}

/**
 * Zoom parameters the original Resizable-Classic layout (root 161) installs through cs2
 * opcodes 6200/6202 before sizing the 3D viewport (`assets/compiled/render/hud/zoom-table.json`,
 * read from the running original client): hop values fy = fg = 127 (`2^(v/256+7)`), limits
 * fu 1 / fz 32767 / fh 1 / fq 32767.
 */
export const FULL_HUD_ZOOM_PARAMETERS = { fy: 127, fg: 127, fu: 1, fz: 32767, fh: 1, fq: 32767 } as const;

/**
 * The original client's full-HUD zoom for a Resizable-Classic viewport of `width` × `height`
 * pixels — the port of `rl.cu` (cs2 6203 `if_setviewport`), the double-precision twin of
 * `client.oh`: hop = fy below 334 px, fg from 434 px, interpolated between; the aspect ratio
 * `height·hop·512 / (width·334)` is clamped to fh..fq (recomputing hop, capped at fz / fu with
 * letterbox bars); zoom = ⌊height·hop / 334⌋. With the layout's parameters this is
 * ⌊height·127/334⌋: 410 at 1920×1080 (the frozen native full-HUD probe), 292 at 1024×768,
 * 547 at 2560×1440. Returns the zoom and the letterboxed viewport rectangle (equal to the input
 * unless the limits engage, which they do not for the layout's stock values).
 */
export function fullHudViewport(width: number, height: number, p = FULL_HUD_ZOOM_PARAMETERS): { zoom: number; x: number; y: number; width: number; height: number } {
  let x = 0, y = 0;
  let w = Math.max(1, Math.trunc(width));
  let h = Math.max(1, Math.trunc(height));
  const n = h - 334;
  let hop: number = n < 0 ? p.fy : n >= 100 ? p.fg : Math.trunc(((p.fg - p.fy) * n) / 100) + p.fy;
  let ratio = (h * hop * 512.0) / (w * 334);
  if (ratio < p.fh) {
    ratio = p.fh;
    hop = (ratio * w * 334.0) / (h * 512);
    if (hop > p.fz) {
      hop = p.fz;
      const inner = (h * hop * 512.0) / (ratio * 334.0);
      const bar = Math.trunc((w - inner) / 2.0);
      x += bar;
      w -= bar * 2;
    }
  } else if (ratio > p.fq) {
    ratio = p.fq;
    hop = (ratio * w * 334.0) / (h * 512);
    if (hop < p.fu) {
      hop = p.fu;
      const inner = (ratio * w * 334.0) / (hop * 512.0);
      const bar = Math.trunc((h - inner) / 2.0);
      y += bar;
      h -= bar * 2;
    }
  }
  return { zoom: Math.trunc((h * hop) / 334.0), x, y, width: w, height: h };
}

/** `fullHudViewport(width, height).zoom`: the full-HUD `RenderCamera.zoom` for a canvas size. */
export function fullHudZoomForViewport(width: number, height: number): number {
  return fullHudViewport(width, height).zoom;
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
