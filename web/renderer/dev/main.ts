/**
 * Developer-only fixture page driving the renderer adapter with the approved fixture scenes and
 * their exact original camera inputs. Not a shell, not a game journey. Exposes
 * `window.__clubscapeBenchmarkV1` (RenderSnapshot protocol) so the browser harness can read
 * genuine GPU-completed frame records, and `window.__clubscapeDev` for the capture script.
 */
import type { DynamicObjectView, RenderFrame, WorldView } from "../../shared/contracts.ts";
import { MINIMAP_STOCK_SCALE, createRenderer, fullHudZoomForViewport, sourceZoomForViewportHeight, type ClubscapeRendererHandle, type MinimapIconPlacements, type PlayerPoseFit, type RendererInstanceLayout } from "../src/index.ts";

/** Per-pose gear fits of the frames drawn so far: counts, worst measures and the failures. */
function summarizePoseFits(fits: PlayerPoseFit[]) {
  const failing = fits.filter((f) => !f.meetsTargets);
  const drawn = [...new Set(fits.map((f) => `${f.slot}:${f.itemId}`))].sort();
  return {
    frames: fits.length,
    precomputed: fits.filter((f) => f.precomputed).length,
    /** Distinct (slot, item) pairs actually drawn — the sequence's `lc.bd` hand overrides applied. */
    drawn,
    maxPenetration: fits.reduce((m, f) => Math.max(m, f.penetration), 0),
    maxGap: fits.reduce((m, f) => Math.max(m, f.gap), 0),
    maxAttachmentGap: fits.reduce((m, f) => Math.max(m, f.attachmentGap), 0),
    maxShift: fits.reduce((m, f) => Math.max(m, f.shift), 0),
    failing: failing.map((f) => ({ sequence: f.sequence, frame: f.frame, itemId: f.itemId, slot: f.slot, penetration: f.penetration, gap: f.gap, attachmentGap: f.attachmentGap })),
  };
}

interface FixtureCamera { scene: string; base: [number, number]; local: [number, number, number]; pitch: number; yaw: number }

/** Original capture inputs (research/reference-pack/v1/manifest.json original_inputs.settings). */
const FIXTURES: FixtureCamera[] = [
  { scene: "lumbridge-castle-plaza", base: [3168, 3168], local: [6912, -1540, 5120], pitch: 2048, yaw: 0 },
  { scene: "lumbridge-river-bridge", base: [3168, 3168], local: [8576, -1540, 4736], pitch: 2048, yaw: 2048 },
  { scene: "tutorial-starting-house", base: [3048, 3056], local: [5888, -2360, 4992], pitch: 2048, yaw: 0 },
  { scene: "tutorial-survival-coast", base: [3048, 3032], local: [7296, -1996, 5632], pitch: 2048, yaw: 1024 },
  { scene: "lumbridge-windmill-route", base: [3120, 3240], local: [6912, -1890, 7040], pitch: 2048, yaw: 1536 },
];

const APPROVED_SOURCE_PACK = "b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d";

interface DevState {
  ready: boolean;
  error: string | null;
  handle: ClubscapeRendererHandle | null;
  frames: RenderFrame[];
  sceneId: string;
  paused: boolean;
}

declare global {
  interface Window {
    __clubscapeDev: DevState & {
      setWorld(world: WorldView | null): void; pause(): void; resume(): void; frameOnce(): Promise<RenderFrame | null>;
      /** Live resize with the source zoom curve for the new height (what a shell does on resize). */
      resizeTo(width: number, height: number): void;
      /** Region mode: move the developer player (camera follows, scene recenters like the original). */
      walkTo?(x: number, y: number): void;
      /** Scenario mode: apply a named developer scenario (see `scenarioWorld`). */
      applyScenario?(name: string): Promise<unknown>;
      /**
       * Scenario mode: the state after frames were drawn — pose fits of the frames actually
       * rendered since the scenario was applied, motion/observer reports and icon placements
       * (`applyScenario` returns the state before its first frame).
       */
      scenarioReport?(): unknown;
      /** Region mode: draw the source minimap surface onto the dev minimap canvas; returns its metadata. */
      minimap?(): unknown;
    };
    __clubscapeBenchmarkV1?: {
      bindRun?(binding: { contractId: string; contractSha256: string }): void;
      read(afterFrame: number | null): unknown;
    };
  }
}

function param(name: string, fallback: string): string {
  return new URLSearchParams(location.search).get(name) ?? fallback;
}

function actorsWorld(fixture: FixtureCamera): WorldView {
  // Developer actors on open ground near the starting house (same tiles as the core test).
  const tile = (x: number, y: number) => ({ x, y, plane: 0 });
  const entity = (id: string, sourceId: number, x: number, y: number, animation: string) => ({
    id, definitionId: `npc:${sourceId}`, sourceId, name: id, kind: "npc" as const, tile: tile(x, y), instance: null,
    hitpoints: 5, maxHitpoints: 5, available: true, animation, actions: [], appearance: {}, equipment: [],
  });
  const goblinTile = fixture.scene === "tutorial-starting-house" ? [3096, 3096] : [fixture.base[0] + 50, fixture.base[1] + 50];
  return {
    revision: "dev", tick: "0",
    player: {
      id: "player-dev", displayName: "dev", appearance: {}, region: fixture.scene, tile: tile(goblinTile[0]! + 2, goblinTile[1]! + 2),
      instance: null, inventory: [], equipment: [], skills: [], hitpoints: 10, prayerPoints: 1, runEnergy: 100, questPoints: 0,
      tutorialStage: "", tutorialInstruction: "", quests: [], unlockedInterfaces: [], activePrayers: [], activity: "idle", animation: "5668", settings: [],
    },
    entities: [entity("goblin-dev", 3028, goblinTile[0]!, goblinTile[1]!, "6181")],
    groundItems: [], dialogue: null, bank: null, shop: null, recovery: null, messages: [],
  };
}

/**
 * Developer scenarios on the tutorial starting-house fixture: live layers and player actions the
 * shell will drive from the authoritative WorldView. Tiles and ids are real source values; the
 * scenarios are not journeys.
 */
function scenarioWorld(name: string): WorldView & { dynamicObjects?: unknown[] } {
  const tile = (x: number, y: number) => ({ x, y, plane: 0 });
  const base = actorsWorld(FIXTURES.find((f) => f.scene === "tutorial-starting-house")!);
  const item = (id: string, sourceId: number, quantity = 1) => ({ id, name: id, quantity, sourceId, iconAsset: null, instanceId: null, charges: null, actions: [] });
  // Motion identity is explicit: the server-bound source sequence in catalog form (what the
  // backend's Player.animation / animation events carry), never derived from the activity here.
  const sequence = (id: number) => `asset.source.osrs.cache2695.sequence.${id}`;
  const player = (x: number, y: number, activity: string, equipment: Array<[string, number, string]>, animation = "") => ({
    ...base.player, tile: tile(x, y), activity, animation,
    equipment: equipment.map(([slot, sourceId, id]) => ({ slot, item: item(id, sourceId) })),
  });
  const npc = (id: string, sourceId: number, x: number, y: number) => ({
    id, definitionId: `asset.source.osrs.cache2695.npc.${sourceId}`, sourceId, name: id, kind: "npc" as const, tile: tile(x, y), instance: null,
    hitpoints: 5, maxHitpoints: 5, available: true, animation: "", actions: [], appearance: {}, equipment: [],
  });
  const object = (id: string, sourceId: number, x: number, y: number, kind: "object" | "temporary_object" = "object") => ({
    id, definitionId: `asset.source.osrs.cache2695.object.${sourceId}`, sourceId, name: id, kind, tile: tile(x, y), instance: null,
    hitpoints: 0, maxHitpoints: 0, available: true, animation: "", actions: [], appearance: {}, equipment: [],
  });
  const FULL_GEAR: Array<[string, number, string]> = [
    ["weapon", 1277, "item.bronze_sword"], ["shield", 1171, "item.wooden_shield"], ["head", 1949, "item.chefs_hat"], ["amulet", 1009, "item.brass_necklace"],
  ];
  switch (name) {
    case "gear-idle":
      return { ...base, player: player(3098, 3098, "idle", [["weapon", 1277, "item.bronze_sword"], ["shield", 1171, "item.wooden_shield"], ["head", 1949, "item.chefs_hat"]]), entities: [npc("guide", 3308, 3096, 3101)] };
    case "gear-fighting":
      return { ...base, player: player(3098, 3098, "fighting", [["weapon", 1277, "item.bronze_sword"], ["shield", 1171, "item.wooden_shield"]], sequence(390)), entities: [npc("rat", 2813, 3099, 3098)] };
    case "woodcutting":
      return { ...base, player: player(3098, 3098, "gathering", [["weapon", 1351, "item.bronze_axe"]], sequence(879)), entities: [object("tree", 1276, 3099, 3098)] };
    case "mining":
      return { ...base, player: player(3098, 3098, "gathering", [["weapon", 1265, "item.bronze_pickaxe"]], sequence(625)), entities: [object("rocks", 10079, 3099, 3098)] };
    case "fishing":
      return { ...base, player: player(3098, 3098, "gathering", [], sequence(621)), entities: [npc("fishing-spot", 3317, 3099, 3098)] };
    case "firemaking":
      return { ...base, player: player(3098, 3098, "producing", [], sequence(733)), entities: [] };
    case "cooking":
      return { ...base, player: player(3098, 3098, "producing", [], sequence(897)), entities: [object("fire", 26185, 3099, 3098, "temporary_object")] };
    case "walking":
      return { ...base, player: player(3098, 3098, "walking", [["weapon", 1351, "item.bronze_axe"]]), entities: [] };
    case "ranged":
      return { ...base, player: player(3098, 3098, "fighting", [["weapon", 841, "item.shortbow"], ["ammo", 882, "item.bronze_arrow"]], sequence(426)), entities: [npc("rat", 2813, 3101, 3098)] };
    case "casting":
      return { ...base, player: player(3098, 3098, "casting", [], sequence(711)), entities: [npc("rat", 2813, 3101, 3098)] };
    case "death":
      return { ...base, player: { ...player(3098, 3098, "idle", [], sequence(836)), hitpoints: 0 }, entities: [] };
    case "gear-death":
      // Full M1 wearable set dying: `lc.bd` hides both hand slots (sword, shield), keeps hat/necklace.
      return { ...base, player: { ...player(3098, 3098, "idle", FULL_GEAR, sequence(836)), hitpoints: 0 }, entities: [] };
    case "gear-woodcutting":
      // Worn sword + shield while chopping: 879 puts the axe in the shield slot and hides the weapon slot.
      return { ...base, player: player(3098, 3098, "gathering", FULL_GEAR, sequence(879)), entities: [object("tree", 1276, 3099, 3098)] };
    case "gear-mining":
      // 625 draws the pickaxe in both hand slots.
      return { ...base, player: player(3098, 3098, "gathering", FULL_GEAR, sequence(625)), entities: [object("rocks", 10079, 3099, 3098)] };
    case "gear-smithing":
      // 898: the hammer (a sequence hand item, not equippable) in the weapon slot, shield hidden.
      return { ...base, player: player(3098, 3098, "producing", FULL_GEAR, sequence(898)), entities: [object("anvil", 2097, 3099, 3098)] };
    case "observer-unbound":
      // game.observer.v1 view: a 2^53+1 tick (string), running false, an action whose animation
      // binding the backend has not published (`animation: null`): reported, never guessed.
      return {
        ...base, tick: "9007199254740993",
        player: {
          ...player(3098, 3098, "producing", FULL_GEAR), running: false, movementTick: null,
          action: {
            version: 1, id: "act-dev-1", activity: "producing", actionId: "action.cooking.cook", target: null, recipeId: "recipe.cooking.shrimps",
            styleId: null, spellId: null, animation: null, startedAtTick: "9007199254740990", cycleStartedAtTick: "9007199254740990",
            nextActionTick: "9007199254740994", observedAtTick: "9007199254740993",
          },
        } as unknown as WorldView["player"],
        entities: [],
      };
    case "ground-items-fire":
      return {
        ...base, player: player(3098, 3098, "idle", []),
        entities: [object("fire", 26185, 3097, 3096, "temporary_object"), npc("goblin", 3028, 3096, 3096)],
        groundItems: [
          { id: "g1", tile: tile(3096, 3099), item: item("item.logs", 1511), canTake: true },
          { id: "g2", tile: tile(3096, 3099), item: item("item.coins", 995, 250), canTake: true },
          { id: "g3", tile: tile(3097, 3100), item: item("item.shrimps", 317), canTake: true },
          { id: "g4", tile: tile(3099, 3100), item: item("item.bronze_axe", 1351), canTake: true },
        ],
      };
    case "door-open":
      return {
        ...base, player: player(3098, 3098, "idle", []), entities: [],
        dynamicObjects: [{ id: "transform.scenery.start_door", objectId: "asset.source.osrs.cache2695.object.9398", sourceId: 9398, tile: tile(3098, 3107), instance: null, state: "object_state.open", doorOpen: true, quarterTurns: 1 }],
      };
    case "roof-player":
      return { ...base, player: player(3094, 3106, "idle", []), entities: [] };
    case "preview":
      return { ...base, player: player(3098, 3098, "idle", [["weapon", 1277, "item.bronze_sword"], ["shield", 1171, "item.wooden_shield"]]), entities: [] };
    // Original dynamic-layer reference cases (assets/reference/osrs240/m1-dynamic): exact case
    // inputs, rendered at the native full-HUD zoom (see applyScenario), which also switches the
    // local player body off because the source frames carry none.
    case "source-door-closed":
    case "source-door-open":
      return {
        ...base, player: { ...player(3094, 3103, "idle", []), animation: sequence(808) }, entities: [],
        dynamicObjects: [{ id: "door-9398", objectId: "asset.source.osrs.cache2695.object.9398", sourceId: 9398, tile: tile(3098, 3107), instance: null, doorOpen: name === "source-door-open", quarterTurns: name === "source-door-open" ? 1 : 0 }],
      };
    case "source-roofs-outside":
    case "source-roofs-hidden":
      return { ...base, player: { ...player(3094, 3099, "idle", []), animation: sequence(808) }, entities: [] };
    case "source-roofs-inside":
      return { ...base, player: { ...player(3094, 3103, "idle", []), animation: sequence(808) }, entities: [] };
    case "unknown-motion":
      // The backend interop gap as it stands today: an action reported with no source
      // animation. The renderer keeps the stance and reports `motion unknown` — never a guess.
      return { ...base, player: player(3098, 3098, "gathering", [["weapon", 1351, "item.bronze_axe"]]), entities: [object("tree", 1276, 3099, 3098)] };
    default:
      throw new Error(`unknown scenario ${name}`);
  }
}

/**
 * Representative live workload for frozen performance measurement (region mode): the geared
 * player, animated NPCs from the source definitions, a fire and ground items around the tile.
 */
function workloadWorld(x: number, y: number, region: string): WorldView {
  const base = devWorld(x, y, region);
  const tile = (tx: number, ty: number) => ({ x: tx, y: ty, plane: 0 });
  const item = (id: string, sourceId: number, quantity = 1) => ({ id, name: id, quantity, sourceId, iconAsset: null, instanceId: null, charges: null, actions: [] });
  const npc = (id: string, sourceId: number, tx: number, ty: number) => ({
    id, definitionId: `asset.source.osrs.cache2695.npc.${sourceId}`, sourceId, name: id, kind: "npc" as const, tile: tile(tx, ty), instance: null,
    hitpoints: 5, maxHitpoints: 5, available: true, animation: "", actions: [], appearance: {}, equipment: [],
  });
  const fire = {
    id: "fire", definitionId: "asset.source.osrs.cache2695.object.26185", sourceId: 26185, name: "Fire", kind: "temporary_object" as const, tile: tile(x - 2, y + 1), instance: null,
    hitpoints: 0, maxHitpoints: 0, available: true, animation: "", actions: [], appearance: {}, equipment: [],
  };
  return {
    ...base,
    player: { ...base.player, activity: "fighting", animation: "asset.source.osrs.cache2695.sequence.390", equipment: [{ slot: "weapon", item: item("item.bronze_sword", 1277) }, { slot: "shield", item: item("item.wooden_shield", 1171) }] },
    entities: [
      npc("goblin-a", 3028, x + 3, y + 2), npc("goblin-b", 3028, x - 4, y + 3), npc("goblin-c", 3028, x + 5, y - 3),
      npc("rat-a", 2813, x - 3, y - 2), npc("rat-b", 2814, x + 2, y - 4), npc("guide", 306, x + 1, y + 5), npc("survival-expert", 8503, x - 6, y),
      fire,
    ],
    groundItems: [
      { id: "g1", tile: tile(x + 1, y + 1), item: item("item.logs", 1511), canTake: true },
      { id: "g2", tile: tile(x + 1, y + 1), item: item("item.coins", 995, 250), canTake: true },
      { id: "g3", tile: tile(x - 1, y - 1), item: item("item.bronze_axe", 1351), canTake: true },
    ],
  };
}

/** Developer WorldView with the penguin player on a tile (region mode). */
/**
 * The canonical M1 Death Office instance: template region 12633 mapped 1:1 onto four 8x8 chunks
 * at origins (3168,5720) (3168,5728) (3176,5720) (3176,5728), plane 0, no turn (the backend's
 * `mechanics.instances` template; the shell forwards it as `instanceLayout`).
 */
const DEATH_OFFICE_LAYOUT: RendererInstanceLayout = {
  template: "instance.template.death-office",
  chunks: [[396, 715], [396, 716], [397, 715], [397, 716]].map(([cx, cy]) => ({
    plane: 0, chunkX: cx!, chunkY: cy!, sourcePlane: 0, sourceChunkX: cx!, sourceChunkY: cy!, quarterTurns: 0,
  })),
};

function devWorld(x: number, y: number, region: string, dynamicObjects: DynamicObjectView[] = [], instanceLayout: RendererInstanceLayout | null = null): WorldView & { instanceLayout: RendererInstanceLayout | null } {
  return {
    revision: "dev", tick: "0", dynamicObjects, instanceLayout,
    player: {
      id: "player-dev", displayName: "dev", appearance: {}, region, tile: { x, y, plane: 0 },
      instance: instanceLayout ? "instance.death-office.dev" : null, inventory: [], equipment: [], skills: [], hitpoints: 10, prayerPoints: 1, runEnergy: 100, questPoints: 0,
      tutorialStage: "", tutorialInstruction: "", quests: [], unlockedInterfaces: [], activePrayers: [], activity: "idle", animation: "5668", settings: [],
    },
    entities: [], groundItems: [], dialogue: null, bank: null, shop: null, recovery: null, messages: [],
  };
}

async function main(): Promise<void> {
  const canvas = document.getElementById("surface") as HTMLCanvasElement;
  const status = document.getElementById("status") as HTMLDivElement;
  const sceneId = param("scene", "tutorial-starting-house");
  const width = Number(param("w", "1920"));
  const height = Number(param("h", "1080"));
  const modelMode = param("mode", "scene") === "model";
  const regionMode = /^region[.:]|^\d{4,5}$|^blocks@/.test(sceneId);
  const fixture = FIXTURES.find((f) => f.scene === sceneId);
  if (!fixture && !modelMode && !regionMode) throw new Error(`unknown fixture scene ${sceneId}`);
  const state: DevState = { ready: false, error: null, handle: null, frames: [], sceneId, paused: false };
  let pending: RenderFrame[] = [];
  window.__clubscapeDev = {
    ...state,
    setWorld(world) { if (world) state.handle?.update(world); },
    pause() { state.paused = true; },
    resume() { state.paused = false; },
    frameOnce: async () => state.handle ? state.handle.frame(performance.now()) : null,
    resizeTo(w, h) {
      const handle = state.handle;
      if (!handle || !fixture) return;
      handle.resize(w, h);
      handle.camera({
        x: fixture.base[0] * 128 + fixture.local[0], height: fixture.local[1], y: fixture.base[1] * 128 + fixture.local[2],
        pitch: fixture.pitch, yaw: fixture.yaw, unitsPerTurn: 16384, zoom: sourceZoomForViewportHeight(h), near: 50, far: 32768,
      });
    },
  };
  const publish = () => Object.assign(window.__clubscapeDev, state);
  window.__clubscapeBenchmarkV1 = {
    read(afterFrame) {
      const diagnostics = state.handle?.diagnostics();
      const frames = afterFrame === null ? [] : pending.filter((f) => f.sequence > afterFrame);
      pending = pending.slice(-16384); // 180 s at 60 Hz fits; the harness reads exact counts from renderedFrames
      const last = diagnostics?.lastFrame ?? null;
      return {
        version: 1, clock: "performance.now", application: "clubscape",
        identity: {
          buildId: "dev-fixture", sceneId, routeId: "fixture-camera", workloadId: "fixture-scene",
          sourcePackSha256: APPROVED_SOURCE_PACK, benchmarkContractSha256: "", assetManifestSha256: diagnostics?.manifestSha256 ?? "", settingsSha256: "",
        },
        ready: state.ready, backend: "webgpu",
        viewport: { width: canvas.width, height: canvas.height, deviceScaleFactor: devicePixelRatio },
        assets: diagnostics?.assets ?? [], entities: {}, deviceEpoch: diagnostics?.deviceEpoch ?? "0",
        renderedFrames: diagnostics?.renderedFrames ?? 0,
        lastSubmittedAtMs: last?.submittedAtMs ?? 0, lastCompletedAtMs: last?.completedAtMs ?? 0, nowMs: performance.now(), frames,
      };
    },
  };
  try {
    const handle = await createRenderer(canvas, {
      assetBaseUrl: "/assets/compiled/render/", manifestUrl: "/assets/compiled/render/manifest.json",
      sourcePackSha256: APPROVED_SOURCE_PACK, width, height,
    }, {
      wasmUrl: "/web/renderer/dist/renderer/pkg/clubscape_renderer_bg.wasm",
      onFrame(frame) { pending.push(frame); state.frames.push(frame); if (state.frames.length > 16384) state.frames.shift(); },
      onDiagnostic(message) { console.warn(`[renderer] ${message}`); },
    });
    state.handle = handle;
    if (modelMode) {
      // Model capture replay: the capture script drives handle.frameModelFixture directly.
      state.ready = true;
      publish();
      return;
    }
    if (regionMode) {
      // Region mode: the world is assembled from blocks around the player; the camera follows a
      // developer player tile (`?px=&py=`) with the source fixture pitch/zoom.
      await handle.loadScene(sceneId);
      const px = Number(param("px", "3222"));
      const py = Number(param("py", "3218"));
      /** Camera offset in source units applied on top of the followed tile (moving workload). */
      let cameraShift = 0;
      let cameraTile = { x: px, y: py };
      const placeCamera = () => {
        handle.camera({
          x: cameraTile.x * 128 + 64 + Math.trunc(cameraShift), height: Number(param("cam_h", "-1540")), y: (cameraTile.y - 8) * 128, pitch: 2048, yaw: Number(param("yaw", "0")),
          unitsPerTurn: 16384, zoom: sourceZoomForViewportHeight(canvas.height), near: 50, far: 32768,
        });
      };
      let instanceLayout: RendererInstanceLayout | null = null;
      const follow = (x: number, y: number) => {
        cameraTile = { x, y };
        cameraShift = 0;
        placeCamera();
        handle.update(devWorld(x, y, sceneId, [], instanceLayout));
      };
      /** Moving-camera workload: the camera glides one tile per 0.6 s (the original walk pace)
       *  back and forth, so every frame re-projects the whole scene (no static replay). */
      let moving = false;
      let movingDirection = 1;
      const advanceCamera = () => {
        if (!moving) return;
        cameraShift += movingDirection * (128 / 36);
        if (Math.abs(cameraShift) >= 128 * 3) movingDirection = -movingDirection;
        placeCamera();
      };
      follow(px, py);
      const minimapCanvas = document.getElementById("minimap") as HTMLCanvasElement;
      /** Draws the source minimap surface of the current scene/plane onto the dev canvas. */
      const showMinimap = () => {
        const surface = handle.minimapSurface();
        minimapCanvas.width = surface.width;
        minimapCanvas.height = surface.height;
        const context = minimapCanvas.getContext("2d")!;
        context.putImageData(surface.pixels, 0, 0);
        // Developer view of the icon layer: each icon's original sprite centred on its tile in
        // the unrotated surface (the HUD applies the full `bo.as` rule with zoom/rotation).
        const sprites = handle.mapIconSprites();
        let drawnIcons = 0;
        for (const icon of surface.icons) {
          const sprite = sprites.get(icon.element);
          if (!sprite) continue;
          const cx = surface.marginX + (icon.x - surface.baseX) * surface.scale + surface.scale / 2;
          const cy = surface.height - surface.marginY - (icon.y - surface.baseY) * surface.scale - surface.scale / 2;
          const layer = document.createElement("canvas");
          layer.width = sprite.width; layer.height = sprite.height;
          layer.getContext("2d")!.putImageData(sprite.pixels, 0, 0);
          context.drawImage(layer, Math.round(cx - sprite.width / 2), Math.round(cy - sprite.height / 2));
          drawnIcons++;
        }
        const { pixels: _pixels, mask, ...meta } = surface;
        let covered = 0;
        for (const m of mask) covered += m;
        // Exact HUD placement of the markers around the developer player for the stock 152x152
        // widget (`client.zr`/`bo.as`): dx/dy in minimap pixels, mask-clipped beyond 50 px.
        const placements = handle.minimapIconPlacements(cameraTile.x, cameraTile.y, MINIMAP_STOCK_SCALE, 152, 152);
        // Cross-check against the pure per-icon helper at the same inputs.
        for (const placed of placements.icons) {
          const sprite = sprites.get(placed.element)!;
          const single = handle.placeMinimapIcon(placed.tileX, placed.tileY, cameraTile.x, cameraTile.y, MINIMAP_STOCK_SCALE, placements.minimapAngle, 152, 152, sprite);
          if (!single || single.x !== placed.x || single.y !== placed.y || single.drawX !== placed.drawX || single.clipped !== placed.clipped) {
            throw new Error(`icon placement mismatch for element ${placed.element} at ${placed.tileX},${placed.tileY}: ${JSON.stringify(single)} vs ${JSON.stringify(placed)}`);
          }
        }
        return { ...meta, covered, drawnIcons, spriteCount: sprites.size, placements };
      };
      window.__clubscapeDev.walkTo = (x: number, y: number) => { follow(x, y); };
      window.__clubscapeDev.minimap = () => showMinimap();
      window.__clubscapeDev.applyScenario = async (name: string) => {
        if (name === "workload" || name === "workload-moving") {
          moving = name === "workload-moving";
          cameraShift = 0;
          placeCamera();
          handle.update(workloadWorld(px, py, sceneId));
          return { fit: handle.playerFitReport(), poseFits: summarizePoseFits(handle.playerPoseFits()), placement: handle.scenePlacement(), movingCamera: moving };
        }
        if (name === "door-open" || name === "door-closed") {
          // Lumbridge castle west large door (source object 12349 at 3213,3221, exported with its
          // four rotations): the developer view turns it a quarter on its own tile.
          handle.update(devWorld(px, py, sceneId, name === "door-open" ? [{ id: "door-dev", objectId: "asset.source.osrs.cache2695.object.12349", sourceId: 12349, tile: { x: 3213, y: 3221, plane: 0 }, instance: null, doorOpen: true, quarterTurns: 1 }] : []));
          return showMinimap();
        }
        if (name === "death-office" || name === "leave-instance") {
          // Enter the Death Office instance (declared chunks only; the adapter reassembles from
          // the template's source square) or return to the ordinary world at the previous tile.
          instanceLayout = name === "death-office" ? DEATH_OFFICE_LAYOUT : null;
          const [x, y] = name === "death-office" ? [3172, 5724] : [px, py];
          follow(x, y);
          // The assembly runs asynchronously (block fetch + assemble); wait until the scene reports
          // the declared layout (or the plain world) before measuring.
          const wanted = instanceLayout ? `#${instanceLayout.template}` : "";
          for (let i = 0; i < 600; i++) {
            const id = handle.diagnostics().sceneId ?? "";
            if (instanceLayout ? id.endsWith(wanted) : !id.includes("#")) break;
            await new Promise((r) => setTimeout(r, 50));
          }
          const d = handle.diagnostics();
          if (instanceLayout && !(d.sceneId ?? "").endsWith(wanted)) throw new Error(`instance assembly did not complete: scene ${d.sceneId}`);
          const minimap = showMinimap();
          return { sceneId: d.sceneId, sceneBase: d.sceneBase, loadedSquares: d.loadedSquares, minimap, tile: { x, y } };
        }
        throw new Error(`region mode knows workload/workload-moving/door-open/door-closed/death-office/leave-instance, not ${name}`);
      };
      state.ready = true;
      publish();
      if (param("status", "0") === "1") status.hidden = false;
      const loop = () => {
        if (!state.paused) {
          advanceCamera();
          handle.frame(performance.now()).then((frame) => {
            if (frame && !status.hidden) {
              const d = handle.diagnostics();
              status.textContent = `${d.sceneId} base=${d.sceneBase?.x},${d.sceneBase?.y} squares=${d.loadedSquares.length} #${frame.sequence} prims=${frame.primitives} gpu=${frame.gpuDurationMs?.toFixed(2) ?? "n/a"}ms`;
            }
          }, (error) => {
            state.error = String(error);
            publish();
            console.error(error);
          });
        }
        if (!state.error) requestAnimationFrame(loop);
      };
      loop();
      return;
    }
    if (!fixture) throw new Error(`unknown fixture scene ${sceneId}`);
    await handle.loadScene(sceneId);
    handle.camera({
      x: fixture.base[0] * 128 + fixture.local[0], height: fixture.local[1], y: fixture.base[1] * 128 + fixture.local[2],
      pitch: fixture.pitch, yaw: fixture.yaw, unitsPerTurn: 16384, zoom: sourceZoomForViewportHeight(height), near: 50, far: 32768,
    });
    if (param("actors", "0") === "1") handle.update(actorsWorld(fixture));
    const previewCanvas = document.getElementById("preview") as HTMLCanvasElement;
    /** Whether the handle currently shows the block-assembled scene at the fixture base (source-* scenarios). */
    let onBlockScene = false;
    /**
     * Original per-placement animated-scenery phases of a Tutorial case (independent source
     * sidecar, read-only `dy.ac` observations before the original draw): replayed on the block
     * scene, which bakes every flame frame. Returns the recorded advance (source cycle −
     * lastUpdate) the frame is drawn at.
     */
    const applySourcePhases = async (caseId: string): Promise<{ frames: number[]; elapsed: number } | null> => {
      const response = await fetch(`/assets/reference/osrs240/m1-dynamic/phases/${caseId}.phases.json`);
      if (response.status === 404) {
        // No sidecar: the case shows no animated flame (roofs outside/inside); the block scene
        // keeps its own phases and the recorded draw cycle (game cycle 0, one cycle of advance).
        handle.setSceneryClock(1);
        return null;
      }
      if (!response.ok) throw new Error(`phase sidecar for ${caseId}: ${response.status}`);
      const sidecar = await response.json() as {
        observations: Array<{
          observation_phase: string; rendering_controller: string; source_object_id: number; world_tile: [number, number, number];
          active_controller: { frame: number; frame_cycle: number }; source_cycle: number; last_update_cycle: number;
        }>;
      };
      const before = sidecar.observations.filter((o) => o.observation_phase === "before-original-draw" && o.rendering_controller === "dy.ac");
      if (before.length === 0) throw new Error(`phase sidecar for ${caseId} has no before-draw dy.ac observations`);
      let elapsed: number | null = null;
      const frames: number[] = [];
      for (const o of before) {
        const [x, y, plane] = o.world_tile;
        const set = handle.setSceneryPhase(plane, x, y, o.source_object_id, o.active_controller.frame, o.active_controller.frame_cycle);
        if (set === 0) throw new Error(`no animated instance of object ${o.source_object_id} at ${x},${y},${plane}`);
        const advance = o.source_cycle - o.last_update_cycle;
        if (elapsed !== null && elapsed !== advance) throw new Error("mixed source clocks in the phase sidecar");
        elapsed = advance;
        frames.push(o.active_controller.frame);
      }
      handle.setSceneryClock(elapsed);
      return { frames, elapsed: elapsed! };
    };
    window.__clubscapeDev.applyScenario = async (name: string) => {
      handle.setRoofMode(name === "roof-player" ? 1 : 0);
      // Scenarios show live rendering: the stock top-plane rule instead of the pinned plane 0.
      handle.setTopPlane(name.startsWith("pinned-") ? 0 : null);
      let sourcePhases: { frames: number[]; elapsed: number } | null = null;
      if (name.startsWith("source-")) {
        // Original dynamic-layer reference inputs: the world assembled from blocks at the
        // fixture's own base (every flame frame baked; proven identical to the fixture scene),
        // the recorded animated-scenery controller states and draw cycle, the locked camera at
        // the native full-HUD zoom (410 at 1920x1080, `fullHudZoomForViewport`), the case's
        // recorded draw plane, the original hide-roofs preference, and — as in the source frames
        // — no local player body (developer fixture controls, not gameplay state).
        if (!onBlockScene) {
          await handle.loadScene(`blocks@${fixture.base[0]},${fixture.base[1]}`);
          onBlockScene = true;
        }
        sourcePhases = await applySourcePhases(name.replace(/^source-/, "tutorial-"));
        handle.camera({
          x: fixture.base[0] * 128 + fixture.local[0], height: fixture.local[1], y: fixture.base[1] * 128 + fixture.local[2],
          pitch: fixture.pitch, yaw: fixture.yaw, unitsPerTurn: 16384, zoom: fullHudZoomForViewport(width, height), near: 50, far: 32768,
        });
        const hidden = name === "source-roofs-hidden" || name.startsWith("source-door");
        handle.setHideRoofs(hidden);
        handle.setTopPlane(hidden ? 0 : 3);
        handle.setHideLocalPlayerBody(true);
      } else {
        if (onBlockScene) {
          await handle.loadScene(sceneId);
          onBlockScene = false;
        }
        handle.setSceneryClock(null);
        handle.setHideRoofs(false);
        handle.setHideLocalPlayerBody(false);
        handle.camera({
          x: fixture.base[0] * 128 + fixture.local[0], height: fixture.local[1], y: fixture.base[1] * 128 + fixture.local[2],
          pitch: fixture.pitch, yaw: fixture.yaw, unitsPerTurn: 16384, zoom: sourceZoomForViewportHeight(height), near: 50, far: 32768,
        });
      }
      handle.update(scenarioWorld(name.replace(/^pinned-/, "")));
      if (name === "preview") {
        // The UI's preview bounds are the 480x315 parent layer; the renderer returns exactly that.
        const image = await handle.framePlayerPreview({ width: 480, height: 315 });
        if (!image) throw new Error("no player body loaded for the preview");
        previewCanvas.width = image.width;
        previewCanvas.height = image.height;
        previewCanvas.getContext("2d")!.putImageData(image, 0, 0);
        let covered = 0;
        for (let i = 3; i < image.data.length; i += 4) if (image.data[i] === 255) covered += 1;
        return { width: image.width, height: image.height, covered, fit: handle.playerFitReport(), poseFits: summarizePoseFits(handle.playerPoseFits()) };
      }
      previewCanvas.width = 0;
      previewCanvas.height = 0;
      currentScenario = name;
      return { ...report(name), sourcePhases };
    };
    let currentScenario = "";
    const report = (name: string) => {
      // Exact minimap marker placement for a stock 152x152 widget around the player (icons of
      // the fixture's plane; an error object when the scene has no map data or sprites).
      let icons: MinimapIconPlacements | { error: string } | null = null;
      if (name.startsWith("gear-") || name === "observer-unbound") {
        const tile = scenarioWorld(name.replace(/^pinned-/, "")).player.tile;
        try {
          icons = handle.minimapIconPlacements(tile.x, tile.y, MINIMAP_STOCK_SCALE, 152, 152);
        } catch (error) {
          icons = { error: String(error) };
        }
      }
      return {
        fit: handle.playerFitReport(), poseFits: summarizePoseFits(handle.playerPoseFits()), placement: handle.scenePlacement(),
        unknownMotions: handle.unknownMotions(), unboundActions: handle.unboundActions(), observerV1: handle.observerV1(), running: handle.playerRunning(), icons,
      };
    };
    window.__clubscapeDev.scenarioReport = () => report(currentScenario);
    state.ready = true;
    publish();
    if (param("status", "0") === "1") status.hidden = false;
    // One frame is issued per animation frame; the adapter lets the next build overlap the
    // previous frame's GPU completion and returns null while its in-flight limit is reached.
    const loop = () => {
      if (!state.paused) {
        handle.frame(performance.now()).then((frame) => {
          if (frame && !status.hidden) {
            status.textContent = `${sceneId} #${frame.sequence} prims=${frame.primitives} cpu=${frame.cpuEncodeMs?.toFixed(1)}ms gpu=${frame.gpuDurationMs?.toFixed(2) ?? "n/a"}ms total=${(frame.completedAtMs - frame.submittedAtMs).toFixed(1)}ms`;
          }
        }, (error) => {
          state.error = String(error);
          publish();
          console.error(error);
        });
      }
      if (!state.error) requestAnimationFrame(loop);
    };
    loop();
  } catch (error) {
    state.error = error instanceof Error ? `${error.message}\n${error.stack ?? ""}` : String(error);
    publish();
    status.hidden = false;
    status.textContent = state.error;
    throw error;
  }
}

void main();
