/**
 * Developer-only fixture page driving the renderer adapter with the approved fixture scenes and
 * their exact original camera inputs. Not a shell, not a game journey. Exposes
 * `window.__clubscapeBenchmarkV1` (RenderSnapshot protocol) so the browser harness can read
 * genuine GPU-completed frame records, and `window.__clubscapeDev` for the capture script.
 */
import type { RenderFrame, WorldView } from "../../shared/contracts.ts";
import { createRenderer, sourceZoomForViewportHeight, type ClubscapeRendererHandle } from "../src/index.ts";

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
  const player = (x: number, y: number, activity: string, equipment: Array<[string, number, string]>) => ({
    ...base.player, tile: tile(x, y), activity, animation: "",
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
  switch (name) {
    case "gear-idle":
      return { ...base, player: player(3098, 3098, "idle", [["weapon", 1277, "item.bronze_sword"], ["shield", 1171, "item.wooden_shield"], ["head", 1949, "item.chefs_hat"]]), entities: [npc("guide", 3308, 3096, 3101)] };
    case "gear-fighting":
      return { ...base, player: player(3098, 3098, "fighting", [["weapon", 1277, "item.bronze_sword"], ["shield", 1171, "item.wooden_shield"]]), entities: [npc("rat", 2813, 3099, 3098)] };
    case "woodcutting":
      return { ...base, player: player(3098, 3098, "gathering", [["weapon", 1351, "item.bronze_axe"]]), entities: [object("tree", 1276, 3099, 3098)] };
    case "mining":
      return { ...base, player: player(3098, 3098, "gathering", [["weapon", 1265, "item.bronze_pickaxe"]]), entities: [object("rocks", 10079, 3099, 3098)] };
    case "fishing":
      return { ...base, player: player(3098, 3098, "gathering", []), entities: [npc("fishing-spot", 3317, 3099, 3098)] };
    case "firemaking":
      return { ...base, player: player(3098, 3098, "producing", []), entities: [] };
    case "cooking":
      return { ...base, player: player(3098, 3098, "producing", []), entities: [object("fire", 26185, 3099, 3098, "temporary_object")] };
    case "walking":
      return { ...base, player: player(3098, 3098, "walking", [["weapon", 1351, "item.bronze_axe"]]), entities: [] };
    case "ranged":
      return { ...base, player: player(3098, 3098, "fighting", [["weapon", 841, "item.shortbow"], ["ammo", 882, "item.bronze_arrow"]]), entities: [npc("rat", 2813, 3101, 3098)] };
    case "casting":
      return { ...base, player: player(3098, 3098, "casting", []), entities: [npc("rat", 2813, 3101, 3098)] };
    case "death":
      return { ...base, player: { ...player(3098, 3098, "idle", []), hitpoints: 0 }, entities: [] };
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
        dynamicObjects: [{ id: "transform.scenery.start_door", objectId: "asset.source.osrs.cache2695.object.9398", tile: tile(3098, 3107), instance: null, state: "object_state.open", doorOpen: true, quarterTurns: 1 }],
      };
    case "roof-player":
      return { ...base, player: player(3094, 3106, "idle", []), entities: [] };
    case "preview":
      return { ...base, player: player(3098, 3098, "idle", [["weapon", 1277, "item.bronze_sword"], ["shield", 1171, "item.wooden_shield"]]), entities: [] };
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
    player: { ...base.player, activity: "walking", equipment: [{ slot: "weapon", item: item("item.bronze_sword", 1277) }, { slot: "shield", item: item("item.wooden_shield", 1171) }] },
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
function devWorld(x: number, y: number, region: string): WorldView {
  return {
    revision: "dev", tick: "0",
    player: {
      id: "player-dev", displayName: "dev", appearance: {}, region, tile: { x, y, plane: 0 },
      instance: null, inventory: [], equipment: [], skills: [], hitpoints: 10, prayerPoints: 1, runEnergy: 100, questPoints: 0,
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
      const follow = (x: number, y: number) => {
        handle.camera({
          x: x * 128 + 64, height: Number(param("cam_h", "-1540")), y: (y - 8) * 128, pitch: 2048, yaw: Number(param("yaw", "0")),
          unitsPerTurn: 16384, zoom: sourceZoomForViewportHeight(canvas.height), near: 50, far: 32768,
        });
        handle.update(devWorld(x, y, sceneId));
      };
      follow(px, py);
      window.__clubscapeDev.walkTo = (x: number, y: number) => { follow(x, y); };
      window.__clubscapeDev.applyScenario = async (name: string) => {
        if (name !== "workload") throw new Error(`region mode only knows the workload scenario, not ${name}`);
        handle.update(workloadWorld(px, py, sceneId));
        return { fit: handle.playerFitReport(), placement: handle.scenePlacement() };
      };
      state.ready = true;
      publish();
      if (param("status", "0") === "1") status.hidden = false;
      const loop = () => {
        if (!state.paused) {
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
    window.__clubscapeDev.applyScenario = async (name: string) => {
      handle.setRoofMode(name === "roof-player" ? 1 : 0);
      // Scenarios show live rendering: the stock top-plane rule instead of the pinned plane 0.
      handle.setTopPlane(name.startsWith("pinned-") ? 0 : null);
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
        return { width: image.width, height: image.height, covered, fit: handle.playerFitReport() };
      }
      previewCanvas.width = 0;
      previewCanvas.height = 0;
      return { fit: handle.playerFitReport(), placement: handle.scenePlacement() };
    };
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
