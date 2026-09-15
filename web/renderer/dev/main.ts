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

async function main(): Promise<void> {
  const canvas = document.getElementById("surface") as HTMLCanvasElement;
  const status = document.getElementById("status") as HTMLDivElement;
  const sceneId = param("scene", "tutorial-starting-house");
  const width = Number(param("w", "1920"));
  const height = Number(param("h", "1080"));
  const modelMode = param("mode", "scene") === "model";
  const fixture = FIXTURES.find((f) => f.scene === sceneId);
  if (!fixture && !modelMode) throw new Error(`unknown fixture scene ${sceneId}`);
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
      pending = pending.slice(-256);
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
      onFrame(frame) { pending.push(frame); state.frames.push(frame); if (state.frames.length > 4096) state.frames.shift(); },
      onDiagnostic(message) { console.warn(`[renderer] ${message}`); },
    });
    state.handle = handle;
    if (modelMode) {
      // Model capture replay: the capture script drives handle.frameModelFixture directly.
      state.ready = true;
      publish();
      return;
    }
    if (!fixture) throw new Error(`unknown fixture scene ${sceneId}`);
    await handle.loadScene(sceneId);
    handle.camera({
      x: fixture.base[0] * 128 + fixture.local[0], height: fixture.local[1], y: fixture.base[1] * 128 + fixture.local[2],
      pitch: fixture.pitch, yaw: fixture.yaw, unitsPerTurn: 16384, zoom: sourceZoomForViewportHeight(height), near: 50, far: 32768,
    });
    if (param("actors", "0") === "1") handle.update(actorsWorld(fixture));
    state.ready = true;
    publish();
    if (param("status", "0") === "1") status.hidden = false;
    const loop = async () => {
      if (!state.paused) {
        try {
          const frame = await handle.frame(performance.now());
          if (frame && !status.hidden) {
            status.textContent = `${sceneId} #${frame.sequence} prims=${frame.primitives} cpu=${frame.cpuEncodeMs?.toFixed(1)}ms gpu=${frame.gpuDurationMs?.toFixed(2) ?? "n/a"}ms total=${(frame.completedAtMs - frame.submittedAtMs).toFixed(1)}ms`;
          }
        } catch (error) {
          state.error = String(error);
          publish();
          console.error(error);
          return;
        }
      }
      requestAnimationFrame(() => { void loop(); });
    };
    void loop();
  } catch (error) {
    state.error = error instanceof Error ? `${error.message}\n${error.stack ?? ""}` : String(error);
    publish();
    status.hidden = false;
    status.textContent = state.error;
    throw error;
  }
}

void main();
