# web/renderer

TypeScript adapter exposing the Rust/WASM WebGPU renderer (`crates/renderer`, feature `web`)
through the shared contracts in `web/shared/contracts.ts`.

The example below is an **explicit recorded-camera diagnostic**, not normal entry.
Normal entry uses `NativeCamera` from the actual protocol/WASM module and the
`cameraSceneReady`, `cameraSource`, `cameraScene` and `applyNativeCamera` extensions.
The canonical region `camera:null`/`controls:null` records remain unchanged.
The source getter currently reports missing native focus and effect inputs rather
than turning rendered tile centres into a fabricated focus.

```ts
import { createRenderer, sourceZoomForViewportHeight } from "../renderer/src/index.ts";

const handle = await createRenderer(canvas, {
  assetBaseUrl: "/assets/compiled/render/",
  manifestUrl: "/assets/compiled/render/manifest.json",
  sourcePackSha256: "b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d",
  width: 1920, height: 1080,
});
await handle.loadScene("tutorial-starting-house");
handle.camera({ x: 3048 * 128 + 5888, height: -2360, y: 3056 * 128 + 4992,
  pitch: 2048, yaw: 0, unitsPerTurn: 16384, zoom: sourceZoomForViewportHeight(1080), near: 50, far: 32768 });
handle.update(worldView);                 // shared WorldView from the shell
const frame = await handle.frame(performance.now()); // resolves after GPU completion
const pick = handle.pick(x, y);           // { kind: "tile" | "entity", ... } | null
```

`createRenderer(canvas, config, options?)` returns `ClubscapeRendererHandle`, a `RendererHandle`
plus:

| Member | Purpose |
| --- | --- |
| `diagnostics()` | Adapter name, scene base/loaded squares, timestamp support, device epoch, manifest SHA, loaded asset ids/hashes, rendered frame count, last frame, device-loss reason. |
| `framePlayerPreview({ width, height, … })` → `Promise<ImageData \| null>` | Model-only penguin preview (current body + worn gear) through the original interface model draw, at exactly the requested native size with coverage alpha; for the UI's `setUiPreview(getUiPreviewBounds())`. Defaults come from the exported interface 679:73 component (`manifest.model_widgets`); `null` until the player body is loaded. Never a reference capture. |
| `setTopPlane(limit \| null)`, `setInstancedMap(bool)` | Top drawn plane. Fixture scenes pin 0 (the approved captures' `dh` argument); region scenes use the stock live rule (`null`). |
| `setRoofMode(bits)`, `setRoofContext(hovered, destination)` | Original roof removal bits (1 player, 2 hovered, 4 destination, 8 camera line); 0 = stock. |
| `playerFitReport()` | Per worn item: bound human label, penguin label, `penetration` (≤ 1), `gap` (≤ 2), `anchorShift`, `designPenetration` (same measure on the human body), retarget scale — source units. |
| `scenePlacement()` | `{ baseX, baseY, sizeTiles: 104, blocks }` — the source scene placement for a dynamic minimap (the UI's minimap raster is keyed by base/plane). |
| `frameModelFixture()` | Developer-only replay of an approved model capture. |
| `cameraSceneReady()` | False during source loading; retained assembly errors throw. Does not assert that native focus/effects exist. |
| `cameraSource()` | Actual actor/world/scene-generation context and rendered placement, plus explicit nullable native focus/effect contracts and missing-input reasons. |
| `cameraScene()` | Original checked corners/settings and rendered paint/model triangles, with source floor-decoration IDs. No guessed heights, geometry generation or camera mechanics. |
| `applyNativeCamera(delivery)` | Rust validates current actor/revision/scene/base and approved provenance, then applies the paired integer native eye/angles and projection. Returns the exact renderer camera for the UI. |

`createRendererContract` is the plain `CreateRenderer` signature. Adapter options:
`wasmUrl`, `onFrame`, `onDiagnostic`, `maxFramesInFlight` (default 2).

## Behaviour

* Requires `navigator.gpu`; rejects with the exact cause when WebGPU, a hardware adapter or the
  device is unavailable. There is no WebGL, 2D-canvas or image fallback.
* Fetches `manifest.json`, checks `kind`/`schema_version` and that
  `approved_reference_pack_sha256 === config.sourcePackSha256`, then fetches palette, the 33
  textures, the NPC animation packs, the 49 skeletal sequences, 27 NPC definitions, the player
  body + human retarget reference, 10 worn item models, 244 dynamic-object variants (doors,
  fire frames, flour bin) and 133 ground-item stack models, verifying each SHA-256 against the
  manifest (≈ 5.7 MB besides scenes/blocks). `loadScene(id)` fetches the scene buffer and model
  pack (gzip twins, decompressed with `DecompressionStream`, both hashes verified). Any mismatch
  throws.
* `camera()` takes `RenderCamera` in world units (128 per tile; `x`/`y` horizontal source axes,
  `height` negative-up, 16384 angle units per turn). Pass `zoom = sourceZoomForViewportHeight(h)`
  to reproduce the client's viewport curve (662 at 1080 px); the renderer never rescales itself.
* `update(world)` serialises the `WorldView` plus the optional `RendererWorldExtensions`
  (`dynamicObjects`, `events`, `instanceLayout`). The player is the approved penguin body (NPC
  2063 / model 21547 at 75/128) wearing the `equipment[].item.sourceId` models with the
  playing sequence's source hand overrides applied (`lc.bd`: death and the eat/spell/fishing/
  smithing family hide the hand slots, woodcutting/mining/fishing/firemaking/smithing put their
  own tool in a hand) and the attachment-preserving bind + per-pose fits (`playerFitReport()`,
  `playerPoseFits()` with `penetration` ≤ 1, surface `gap` ≤ 2 and `attachmentGap` ≤ 2 targets;
  the recorded `gear/pose-fits.json` schema 2 is loaded with the assets so no frame solves at
  draw time — the gate over all legal item-frames is currently unmet, see
  `crates/renderer/README.md` "Known deviations"; `meetsTargets: false` entries are exact
  failures, never hidden). **Motion
  identity is explicit only** (the original client plays what the server sends and never derives
  actions locally), in this precedence: `player.animation` / `entity.animation` as a bare id
  (`"879"`) or the catalog id (`asset.source.osrs.cache2695.sequence.879`); the
  **`game.observer.v1` fields** — `running` / `movementTick` are the movement actually executed
  this tick (a final exhausted run step is running; a later non-moving tick is not; explicit
  values win over any inference) and `action: ActorActionView` plays its bound `animation`
  anchored on `cycleStartedAtTick` at 600 ms per tick, keeping its clock across polls and
  reconnects for the same `id`/cycle, re-anchoring on a new cycle, ending on `action: null`,
  and with `animation: null` keeping the stance while reporting the explicit action
  (`motion unknown: <actor>: action <id> (<activity>, <actionId>) has no bound source
  animation`) — never a nearby-object or default motion; the latest animation event for that
  actor (`events[]`, mirroring the protocol `Event{kind:"animation", actorId, animationAsset,
  eventId}`; it plays until it ends, a newer event arrives, the actor moves or the activity
  returns to rest); else the movement stance: stand 808, walk 819 or run 824 — for observers
  without `running`, by the original two-tiles-per-`tick` rule (the `run` setting decides only
  in-between or without ticks). `observerV1()` reports whether the last view carried the
  observer fields. An `activity` such as `gathering`/`producing`/`fighting`/`casting`, or
  `hitpoints === 0`, with no source animation keeps the stance and is reported as
  `motion unknown: …` through `onDiagnostic` / `unknownMotions()`, never a guessed pose;
  observer-v1 actions with `animation: null` are additionally listed with their exact
  `id`/`activity`/`actionId`/`recipeId`/`styleId`/`spellId` by `unboundActions()` for the
  backend's binding work. Observer ticks (`tick`, `movementTick`, `cycleStartedAtTick`) are
  decoded losslessly as decimal `u64` strings — a malformed one on an observer view makes
  `update()` throw before any state changes (no silent clock reset). NPC
  entities with a source definition play the definition's own stand/walk sequences locally
  exactly as the original client does; their deaths and actions are server animations too.
  `temporary_object` entities (fire 26185) animate through the source frames, `groundItems`
  draw the tile's top three stacks (quantity variants), and `dynamicObjects` swap door walls —
  selected through the shell's validated `sourceId` only (an entry without one is reported and
  not drawn; `objectId` strings are never parsed). Entities in another `instance` than the
  player are skipped; unknown ids or missing models are reported, never invented. The
  activity-based table (`developerMotionFallback`, `set_motion_fallback`) exists for developer
  fixtures only.
* `instanceLayout` (`RendererInstanceLayout`: the backend template identity plus its validated
  `GenericInstanceChunkMapping` entries `{plane, chunkX, chunkY, sourcePlane, sourceChunkX,
  sourceChunkY, quarterTurns}`, forwarded by the shell): while set, region scenes are assembled
  from the declared chunks only — each source chunk copied to its destination chunk, every other
  chunk unloaded (no terrain, scenery, picks or minimap data, like the original template loader)
  — block fetches shrink to the declared source squares, and a changed layout (or `null`, back
  to the ordinary world) reassembles on the next `update()`. The renderer never infers a layout
  from an instance id. `quarterTurns !== 0` is rejected with a diagnostic naming the mapping
  (block exports hold lit placed geometry that cannot be turned exactly).
* `frame(nowMs)` returns `null` before a scene is loaded or while `maxFramesInFlight` (default 2)
  frames are pending, otherwise a `RenderFrame` whose `completedAtMs` is taken after the WebGPU
  queue's submitted-work-done signal (`gpuDurationMs` from timestamp queries when supported;
  omitted otherwise). With two frames in flight the next frame's CPU build overlaps the previous
  frame's GPU completion; every record still describes its own submission. Issue one `frame()`
  per animation frame and do not await it before scheduling the next tick. Uncaptured WebGPU
  errors, pipeline validation failures and device loss reject the promise.
* Two original projections exist: the frozen viewport-only scene fixtures use the stock
  `client.oh` curve (`sourceZoomForViewportHeight`: 662 at 1080 px, fy 256 / fg 205), the
  composed Resizable-Classic HUD uses the layout scripts' parameters (fy = fg = 127 through cs2
  6200, read from the running original client into `hud/zoom-table.json`):
  `fullHudZoomForViewport(width, height)` / `fullHudViewport()` port `rl.cu` and give
  ⌊height·127/334⌋ — 410 at 1920×1080, 292 at 1024×768, 273 at 1280×720, 547 at 2560×1440 —
  pinned to the original's own value at 16 canvas sizes (`crates/renderer/tests/hud_zoom.rs`).
  These are diagnostic helper paths. The approved normal controller instead computes projection
  from its own constructor FOV256/205 (662 at1080) and later native wheel state; its initial
  numeric result does not make the viewport-only fixture eye a normal-entry camera.
* `pick(x, y)` replays the last frame's exact fill coverage and returns the topmost tile or
  entity; scenery carries `scenery.objectId`/`type` (placement type), item piles resolve to
  their tile (the WorldView lists the items there). It never mutates state.
* `resize(w, h)` resizes the canvas and surface. Diagnostics supply their matching camera zoom;
  normal entry resizes `NativeCamera` and applies its next native output. Unsupported native
  letterboxing is an explicit integration error, not a scalar zoom approximation.
* Region scenes: `loadScene("region.osrs.12850")` (content region id) or a bare map square id
  assembles the world from the manifest's blocks around that square, exactly as the original
  builds a 104×104 scene around the player's chunk. Afterwards `update(world)` recenters when the
  player comes within 16 tiles of the scene edge (`base = ((tile >> 3) - 6) * 8`), streaming the
  needed squares (gzip, hash-verified) and dropping distant ones; `diagnostics().sceneBase` /
  `loadedSquares` expose the state. Squares outside the exported world stay empty, as unloaded
  map squares do in the original. Frames rendered while the player's tile is outside the loaded
  scene report `entities skipped` rather than drawing a substitute. Each streamed square also
  fetches its minimap sidecar (`minimap/blocks/<square>.bin`) and, once, the map-scene sprite
  asset (`minimap/mapscenes.bin`).
  Source loads are coalesced by base and actual instance-layout identity. A superseded load
  cannot install geometry for a newer actor/world; frames wait while loading, and source
  errors remain errors. Teardown aborts asset requests and invalidates pending scene publication.
* `minimapSurface()` returns the **source minimap** of the current scene on the player's
  plane: the original `client.bm(world, 512×512, 4.0, plane, 0, 0, 48, 48)` sweep ported to
  Rust and drawn over the streamed blocks with the current door states (`dynamicObjects`) —
  terrain shapes and colours, bridge tiles, wall marks (red interactive/door, white plain),
  diagonal walls and map-scene sprites — as `MinimapSurface {pixels: ImageData, mask, width,
  height, scale: 4, marginX/Y: 48, baseX, baseY, plane, revision, complete, stats, notes,
  icons}`. It is redrawn only when the scene, plane or door states change (`revision`), so the
  UI can cache by revision. Scene tile (x, y) covers raster x `48 + (x − baseX) * 4` and y
  `512 − 48 − (y − baseY + 1) * 4`; the UI's `MinimapPainter.project(widget, image,
  (player.x − baseX) * 4 + 50, 462 − (player.y − baseY) * 4)` centring applies unchanged with
  `baseX/baseY` taken from the surface instead of a static raster's catalogue entry. `mask` is
  1 where the original drew map data (0 where no tile exists — those pixels hold the source
  fill value `0x000001`). `icons` lists the map icons of the plane exactly as the original
  `bu.aa` pass collects them (floor decorations whose object definition names a map element the
  original shows), and `sourceIconMismatches` compares that list with the original pass recorded
  per square by the export (0 on all 15 native cases). `mapIconSprites()` returns the original
  map-element sprites (`minimap/mapicons.bin`, 386 elements, fetched with the map-scene asset)
  as `Map<element, MapIconSprite {pixels: ImageData, width, height, …}>`; the HUD draws them
  over the surface per frame with the original `client.zr`/`bo.as` rule, computed exactly by
  `placeMinimapIcon(...)` / `minimapIconPlacements(playerTileX, playerTileY, scale, widgetW,
  widgetH)` → `MinimapIconPlacement {x, y, drawX, drawY, clipped, dx, dy}` in **minimap pixels**
  (`MINIMAP_STOCK_SCALE` = 1/32: 4 px per tile; cut-off 80 px = 20 tiles, mask-clipped beyond
  50 px; the minimap angle is the camera yaw in 16384 units) — the earlier prose that read the
  6400/2500 thresholds as fine units was wrong (that would drop markers beyond ~0.6 tile);
  the placement doc on `MinimapSurface.icons` is now the pixel rule (offset from the player's fine position, minimap zoom, map rotation,
  80-unit radius). The call throws when no scene, no map-scene asset or — with `complete: false`
  and `notes` — when a square's sidecar is missing; it never returns a blank, static or
  approximate map. Native comparison: all 15 approved minimap rasters match pixel-exactly over
  the full 512x512 surface, including the outer five-tile band, with identical icon lists.
  Assembly-time source terrain blending preserves the scene-dependent edge colours.

## Building

```bash
web/renderer/build.sh
```

Builds `crates/renderer` for `wasm32-unknown-unknown` (release, feature `web`), runs
`wasm-bindgen --target web` (CLI 0.2.128, checked against the crate pin) into `pkg/`, optionally
`wasm-opt -O2`, compiles the TypeScript with `web/node_modules/.bin/tsc` into `dist/` and copies
the generated JS/WASM alongside. `pkg/*.d.ts` are committed so shells can type-check without a
Rust toolchain; `pkg/*.js`, `pkg/*.wasm` and `dist/` are generated.

Dependencies: `pnpm install` in `web/` (TypeScript 7, playwright-core), Rust 1.98 with the
wasm32 target, wasm-bindgen CLI 0.2.128, optional `wasm-opt`.

## Developer fixture page and browser capture

```bash
# from the repository root, after build.sh
xvfb-run --auto-servernum --server-args="-screen 0 2720x1600x24 -nolisten tcp" \
  node web/renderer/dev/capture.ts --out .local/render-assets/browser/run1 --measure-ms 10000
.local/render-assets/venv/bin/python tools/render-assets/compare.py --batch \
  .local/render-assets/browser/run1 assets/reference/osrs240/scenes --diff-dir .local/render-assets/browser/run1/diff
.local/render-assets/venv/bin/python tools/render-assets/compare.py --batch \
  .local/render-assets/browser/run1/models assets/reference/osrs240/models
```

`dev/serve.ts` serves the repository read-only on 127.0.0.1; `dev/index.html` + `dev/main.ts`
drive the adapter with the approved fixture cameras (`?scene=<id>&w=&h=&actors=1`,
`?scene=region.osrs.12850&px=&py=` for the streamed world, or `?mode=model` for capture replays)
and expose `window.__clubscapeBenchmarkV1` (RenderSnapshot protocol) and `window.__clubscapeDev`
(`applyScenario(name)` for the developer live-layer scenarios). `dev/capture.ts` launches the
pinned windowed Chrome for Testing with the sandbox on and the `sparky-vulkan-x11` graphics
arguments from `docs/machines/sparky.md` (no `--no-sandbox`, no software renderer),
screenshots the visible canvas element for the five scenes, streams the world across a
map-square edge, replays the 58 approved model captures, runs the scenarios (gear, every
required action motion, ground items + fire, door state, roof mode, interface preview readback;
`--scenarios 0` skips; `--scenario-filter a,b` selects; `source-*` scenarios reproduce the
Tutorial dynamic-layer reference inputs exactly — the block scene at the fixture base, the
source phase sidecar's `dy.ac` controller states and one-cycle advance
(`setSceneryPhase`/`setSceneryClock`), native full-HUD zoom from `fullHudZoomForViewport`, no
local player body — and match all five original frames over every scene pixel on Sparky),
checks 1024×768 / 1280×720 / 2560×1440, a live resize and picks, and
writes `report.json` with GPU-completed frame statistics. `--workload-ms N` adds a frozen
representative workload (streamed Lumbridge scene, geared fighting player, 7 animated NPCs, fire,
ground items; ≈ 69k primitives) with the camera held on the player; `--workload-moving 1` glides
the camera one tile per 0.6 s instead (the original walk pace), so every frame re-projects the
whole scene; `--uncapped 1` removes Chrome's 60 Hz compositor pacing to measure the throughput
ceiling (frames are still presented). Each frame record carries `cpuBreakdownMs`
(`build` = traversal + projection, `pack` = GPU layout + bins, `upload`, `present`).

Performance work in the renderer: static placements (scene models at fixed positions) are
projected once per camera signature and replayed while the camera holds still (the traversal
and every draw decision still run each frame; `tests/static_cache.rs` proves the stream equals a
fresh build, actors moving included, and that a camera move drops the cache); the model face
ordering and the GPU packing no longer allocate per model/frame. Nothing is culled, scaled or
skipped: primitives, resolution and draw distance are unchanged.

Latest Sparky results (Chrome 153, Xvfb, NVIDIA GB10, 1920×1080; not an owner/Mac/Edge
acceptance): every scene and model capture identical to the source PNGs (2073600/2073600
pixels). Frozen workload at the display cadence, with the per-pose gear fit table, observer
fields and icon layer in place — 30 s: 1800 GPU-completed frames, 59.97 fps between the first
and last completion (59.66 fps over the harness window, which includes the harness's own
wait/evaluate overhead), CPU p50 7 / p95 8 ms (build 3 / pack 3 / upload 1), GPU p95 3.0 ms,
completion gap p95 21 ms, max 54 ms, 1 gap above 33.4 ms (run under a host load average of
10 from other workers); 180 s: 10802 frames, 60.00 fps between completions (59.77 over the
window), CPU p50 7 / p95 8 ms, GPU p95 2.9 ms, gap p95 18 ms, max 33 ms, 0 of 10801 gaps above
33.4 ms. Moving camera, 30 s: 1801 frames, 59.97 fps between completions (59.60 window), CPU
p50 7 / p95 12 ms (build 5), GPU p95 3.4 ms, gap p95 28 ms, max 36 ms, 6 gaps above 33.4 ms
(host load average 5; the earlier quiet-host run measured gap p95 20 ms, 1 gap) — this case
is **below** the frozen gap target and is reported failing. Re-measured after the attachment
fit / hand-override / observer work (moving camera, 30 s): 1802 frames, 60.02 fps between
completions (59.68 over the harness window), CPU p50 7 / p95 10 / max 13 ms (build 4 / 7 / 8,
pack 2, upload 1), GPU p95 3.5 ms, submit→complete max 18 ms, gap p95 19 ms, max 34 ms, 2 gaps
above 33.4 ms — within the gap target on this run but still two dropped frames, so not a ≥ 60
window and not a pass. Earlier quiet-host runs: 30 s 60.01
fps / gap p95 18 ms / 0 gaps; uncapped throughput ceiling 73.5 fps (static camera) and 99.1 fps
(moving camera; the static case is bounded by main-thread completion scheduling, not by CPU or
GPU time). Before the static-placement cache the same workload measured 59.85 fps with gap p95
28–30 ms and 61 gaps above 33.4 ms per 180 s. These remain Sparky engineering measurements of
genuine GPU-completed frames — the display cadence is Chrome's 60 Hz compositor under Xvfb, so
"60.00 fps" is the cadence with no dropped frame, not a renderer ceiling — and are not the
frozen ≥ 60 fps / gap-p95 contract proof on the owner's hardware; none of the window fps
figures (59.60–59.77) reaches 60.

## Contract notes for the shell

* Scene ids: the fixture names `lumbridge-castle-plaza`, `lumbridge-river-bridge`,
  `tutorial-starting-house`, `tutorial-survival-coast`, `lumbridge-windmill-route`
  (`FIXTURE_SCENE_IDS`, static captures) and region ids `region.osrs.<square>` /
  `regionSceneId(square)` for the streamed world (`PlayerView.region` uses the same form).
* `ScenePick` for scenery carries an extra `scenery` object (`objectId`, `type`, `spanX`,
  `spanY`) beside the contract fields; the contract itself is unchanged.
* `RendererConfig` needs no change. Optional additions that would help the shell but were **not**
  made to the shared file: a `zoom` derivation helper (provided here as
  `sourceZoomForViewportHeight`) and a diagnostics accessor (provided as the
  `ClubscapeRendererHandle` extension).
* **Data the renderer consumes from backend/shell** (the `game.observer.v1` contract plus the
  typed `RendererWorldExtensions`):
  1. `game.observer.v1`: `PlayerView.running` / `movementTick` and `action: ActorActionView`
     (also on visible-player `EntityView`s) — the executed movement and the authoritative action
     instance/phase; `action.animation` must carry the source sequence identity
     (`asset.source.osrs.cache2695.sequence.<id>` or `<id>`) for the motion to play, otherwise
     the renderer keeps the stance and reports the explicit action as `motion unknown`.
     Missing observer fields mean an older observer (`observerV1() === false`): movement falls
     back to the tick rule, actions to (2)/(3).
  2. `PlayerView.animation` / `EntityView.animation` with the source sequence identity for
     server-played motions (deaths, NPC actions).
  3. `events?: RendererAnimationEvent[]` — legacy alternative when the shell forwards protocol
     `Event`s of kind `animation` (`actorId`, `animationAsset`, `eventId`).
  4. `WorldView.dynamicObjects` (`DynamicObjectView`) — door states with the shell's validated
     `sourceId`; entries without one are reported and not drawn (no id-string parsing).
  5. `instanceLayout?: RendererInstanceLayout | null` — the backend instance template identity
     and its validated chunk mappings while the player is inside an instance (the M1 Death
     Office template: four turn-0 8×8 mappings of region 12633); `null` outside.
  6. `PlayerView.settings` `{ setting: "run", enabled }` and a monotonic `WorldView.tick` — used
     only by the legacy movement inference when `running` is absent.
* Interface preview: call `framePlayerPreview({ width, height })` with the UI's
  `getUiPreviewBounds()` size (480×315 for the character creator) after `update(world)` so the
  worn gear matches, and hand the `ImageData` to `setUiPreview()` via a canvas of the same size.
  The renderer sways the model with the original 20 ms cycle, so re-request per animation frame
  while the interface is open.
* Terrain: with the first world block the adapter also loads `terrain/floors.bin`
  (`manifest.floor_definitions`); every block scene is then rebuilt by the original terrain
  pass at its own base (`diagnostics().terrainRebuilt` reports the paint / shaped-tile counts;
  `null` means no block scene is loaded). Missing definitions, referenced floors or a block's
  raw terrain fail explicitly without replacing the current scene. Legacy lit-only blocks
  cannot be used to substitute known-inexact edge colours. Floor definitions load independently
  of the optional fixture minimap sprites.
* Minimap: call `minimapSurface()` when the UI paints its minimap widget (cheap when the
  `revision` is unchanged) and give `pixels`, `baseX`, `baseY`, `plane` and `mask` to the
  UI's painter in place of the static `ui/minimaps/<base>-<plane>.png` catalogue raster. The
  surface follows the streamed scene (recentering changes `baseX/baseY`), the player's plane
  and door states; entity dots and the player marker remain the UI's layer over it, and the map
  icons are drawn by the UI from `surface.icons` with `mapIconSprites()` (original sprites) at
  the positions `minimapIconPlacements()` returns (the exact `bo.as` rule; plain blit inside
  50 px adding the sprite's own offsets, mask-clipped blit beyond it without them).
* World blocks are streamed by manifest key from `assetBaseUrl` (`blocks/<square>.bin.gz`,
  `blocks/<square>.models.bin.gz`, `minimap/blocks/<square>.bin`, `minimap/mapscenes.bin`,
  `minimap/mapicons.bin`); `terrain/floors.bin`, `gear/pose-fits.json` and
  `hud/zoom-table.json` are published.
  They are not committed: host the members of the deterministic pack described in
  `assets/compiled/render/blocks.index.json` (`tools/render-assets/README.md`, "World block
  package") under the same base URL; the adapter verifies every gzip and inflated hash and
  fails explicitly on a mismatch or missing square that the manifest lists.
