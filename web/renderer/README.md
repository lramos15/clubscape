# web/renderer

TypeScript adapter exposing the Rust/WASM WebGPU renderer (`crates/renderer`, feature `web`)
through the shared contracts in `web/shared/contracts.ts`.

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
  (`dynamicObjects`, `events`). The player is the approved penguin body (NPC 2063 / model 21547
  at 75/128) wearing the `equipment[].item.sourceId` models. **Motion identity is explicit
  only** (the original client plays what the server sends and never derives actions locally):
  the sequence comes from `player.animation` / `entity.animation` as a bare id (`"879"`) or the
  catalog id (`asset.source.osrs.cache2695.sequence.879`), else from the latest animation event
  for that actor (`events[]`, mirroring the protocol `Event{kind:"animation", actorId,
  animationAsset, eventId}`; it plays until it ends, a newer event arrives, the actor moves or
  the activity returns to rest), else the movement stance: stand 808, walk 819 or run 824 —
  running by the original rule (two tiles in one server `tick`; the `run` setting decides only
  when several ticks elapsed with an in-between step count, or when no tick is available). An
  `activity` such as `gathering`/`producing`/`fighting`/`casting`, or `hitpoints === 0`, with no
  source animation keeps the stance and is reported as `motion unknown: …` through
  `onDiagnostic` / `unknownMotions()` — the current backend interop gap (`Player.animation` is
  empty), never a guessed pose. NPC entities with a source definition play the definition's
  own stand/walk sequences locally exactly as the original client does; their deaths and
  actions are server animations too. `temporary_object` entities (fire 26185) animate through
  the source frames, `groundItems` draw the tile's top three stacks (quantity variants), and
  `dynamicObjects` swap door walls. Entities in another `instance` than the player are
  skipped; unknown ids or missing models are reported, never invented. The activity-based
  table (`developerMotionFallback`, `set_motion_fallback`) exists for developer fixtures only.
* `frame(nowMs)` returns `null` before a scene is loaded or while `maxFramesInFlight` (default 2)
  frames are pending, otherwise a `RenderFrame` whose `completedAtMs` is taken after the WebGPU
  queue's submitted-work-done signal (`gpuDurationMs` from timestamp queries when supported;
  omitted otherwise). With two frames in flight the next frame's CPU build overlaps the previous
  frame's GPU completion; every record still describes its own submission. Issue one `frame()`
  per animation frame and do not await it before scheduling the next tick. Uncaptured WebGPU
  errors, pipeline validation failures and device loss reject the promise.
* Two original projections exist at 1080 px: the frozen viewport-only scene fixtures use zoom
  662 (`sourceZoomForViewportHeight`), the composed full-HUD frames of the dynamic-layer
  references use the native full-HUD zoom 410. The shell must pass the zoom of the composition it
  is reproducing; the renderer applies exactly the zoom it is given.
* `pick(x, y)` replays the last frame's exact fill coverage and returns the topmost tile or
  entity; scenery carries `scenery.objectId`/`type` (placement type), item piles resolve to
  their tile (the WorldView lists the items there). It never mutates state.
* `resize(w, h)` resizes the canvas, surface and projection; call `camera()` again with the new
  zoom.
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
  fill value `0x000001`). `icons` lists the map-element ids of floor decorations on the plane
  (what the original minimap widget draws as map icons; their sprites are UI catalogue data).
  The call throws when no scene, no map-scene asset or — with `complete: false` and `notes` —
  when a square's sidecar is missing; it never returns a blank, static or approximate map.
  Native comparison: all 15 approved minimap rasters match pixel-exactly over the scene
  interior; the outer 5-tile band of a scene differs (base-dependent floor blending, see
  `crates/renderer/README.md`, "Known deviations").

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
`--scenarios 0` skips), checks 1024×768 / 1280×720 / 2560×1440, a live resize and picks, and
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
pixels). Frozen workload at the display cadence — 30 s: 1802 GPU-completed frames, 60.01 fps
between the first and last completion (59.67 fps over the harness window, which includes the
harness's own wait/evaluate overhead), CPU p50 5 / p95 8 ms (build 3 / pack 2 / upload 1),
GPU p95 2.6 ms, completion gap p95 18 ms, max 28 ms, 0 gaps above 33.4 ms; 180 s: 10801
frames, 60.00 fps between completions (59.75 over the window), CPU p50 6 / p95 8 ms, GPU p95
2.9 ms, gap p95 20 ms, max 37 ms, 1 of 10800 gaps above 33.4 ms. Moving camera, 30 s: 1800
frames, 59.96 fps between completions, CPU p50 7 / p95 10 ms (build 5), gap p95 20 ms, max
35 ms, 1 gap above 33.4 ms, 59.5k–81.5k primitives. Uncapped throughput ceiling, 30 s: 73.5 fps
(static camera) and 99.1 fps (moving camera; the static case is bounded by main-thread
completion scheduling, not by CPU or GPU time). Before this work the same workload measured
59.85 fps with gap p95 28–30 ms and 61 gaps above 33.4 ms per 180 s. These remain Sparky
engineering measurements of genuine GPU-completed frames — the display cadence is Chrome's
60 Hz compositor under Xvfb, so "60.00 fps" is the cadence with no dropped frame, not a
renderer ceiling — and are not the frozen ≥ 60 fps / gap-p95 contract proof on the owner's
hardware; the 180 s gap p95 sits exactly at 20 ms.

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
* **Data the renderer needs from backend/shell** (typed in `RendererWorldExtensions` /
  `RendererAnimationEvent` / `RendererDynamicObject`; the frozen contract already has the
  first two channels):
  1. `PlayerView.animation` and `EntityView.animation` filled with the source sequence identity
     (`asset.source.osrs.cache2695.sequence.<id>` or `<id>`) for every action, combat swing and
     death the server plays — today the backend sends `""` (interop gap) and the renderer
     reports `motion unknown`.
  2. `PlayerView.settings` containing `{ setting: "run", enabled }` and a monotonically
     increasing `WorldView.tick` per server tick (both already in the contract) — running is
     derived from tiles per tick like the original.
  3. `events?: RendererAnimationEvent[]` — optional alternative to (1) when the shell forwards
     protocol `Event`s of kind `animation` (`actorId`, `animationAsset`, `eventId`).
  4. `dynamicObjects?: RendererDynamicObject[]` — the protocol `WorldSnapshot.dynamic_objects`
     with catalog-resolved `objectId` (`asset.source.osrs.cache2695.object.<id>`) or numeric
     `sourceId`, `tile`, `instance`, `doorOpen`, `quarterTurns`, for door states.
* Interface preview: call `framePlayerPreview({ width, height })` with the UI's
  `getUiPreviewBounds()` size (480×315 for the character creator) after `update(world)` so the
  worn gear matches, and hand the `ImageData` to `setUiPreview()` via a canvas of the same size.
  The renderer sways the model with the original 20 ms cycle, so re-request per animation frame
  while the interface is open.
* Minimap: call `minimapSurface()` when the UI paints its minimap widget (cheap when the
  `revision` is unchanged) and give `pixels`, `baseX`, `baseY`, `plane` and `mask` to the
  UI's painter in place of the static `ui/minimaps/<base>-<plane>.png` catalogue raster. The
  surface follows the streamed scene (recentering changes `baseX/baseY`), the player's plane
  and door states; entity dots, the player marker and map-element icon sprites remain the
  UI's layer over it.
* World blocks are streamed by manifest key from `assetBaseUrl` (`blocks/<square>.bin.gz`,
  `blocks/<square>.models.bin.gz`, `minimap/blocks/<square>.bin`, `minimap/mapscenes.bin`).
  They are not committed: host the members of the deterministic pack described in
  `assets/compiled/render/blocks.index.json` (`tools/render-assets/README.md`, "World block
  package") under the same base URL; the adapter verifies every gzip and inflated hash and
  fails explicitly on a mismatch or missing square that the manifest lists.
