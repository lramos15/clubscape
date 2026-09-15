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
| `playerFitReport()` | Per worn item: bound human label, penguin label, anchor gap, deepest penetration (source units), retarget scale. |
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
* `update(world)` serialises the `WorldView` (optionally extended with `dynamicObjects`, see
  below). The player is the approved penguin body (NPC 2063 / model 21547 at 75/128) wearing the
  `equipment[].item.sourceId` models, animated from `activity` (walk 819, woodcutting 879,
  mining 625, net fishing 621, firemaking 733, cooking 897/896, smelting 899, smithing 898,
  melee 386/390/422, shortbow 426, wind strike 711, death 836 when `hitpoints` is 0, idle 808),
  with the object/NPC next to the player deciding between gathering/producing variants. NPC
  entities with a source definition play their own stand/walk/death motions; goblin/penguin
  fixtures may still use the baked `animation` ids. `temporary_object` entities (fire 26185)
  animate through the source frames, `groundItems` draw the tile's top three stacks (quantity
  variants), and `dynamicObjects` swap door walls. Entities in another `instance` than the
  player are skipped; unknown ids or missing models are reported through
  `options.onDiagnostic`, never invented.
* `frame(nowMs)` returns `null` before a scene is loaded or while `maxFramesInFlight` (default 2)
  frames are pending, otherwise a `RenderFrame` whose `completedAtMs` is taken after the WebGPU
  queue's submitted-work-done signal (`gpuDurationMs` from timestamp queries when supported;
  omitted otherwise). With two frames in flight the next frame's CPU build overlaps the previous
  frame's GPU completion; every record still describes its own submission. Issue one `frame()`
  per animation frame and do not await it before scheduling the next tick. Uncaptured WebGPU
  errors, pipeline validation failures and device loss reject the promise.
* `pick(x, y)` replays the last frame's exact fill coverage and returns the topmost tile or
  entity (`id` is the scene object hash or the entity id hash). It never mutates state.
* `resize(w, h)` resizes the canvas, surface and projection; call `camera()` again with the new
  zoom.
* Region scenes: `loadScene("region.osrs.12850")` (content region id) or a bare map square id
  assembles the world from the manifest's blocks around that square, exactly as the original
  builds a 104×104 scene around the player's chunk. Afterwards `update(world)` recenters when the
  player comes within 16 tiles of the scene edge (`base = ((tile >> 3) - 6) * 8`), streaming the
  needed squares (gzip, hash-verified) and dropping distant ones; `diagnostics().sceneBase` /
  `loadedSquares` expose the state. Squares outside the exported world stay empty, as unloaded
  map squares do in the original. Frames rendered while the player's tile is outside the loaded
  scene report `entities skipped` rather than drawing a substitute.

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
representative workload (streamed Lumbridge scene, geared walking player, 7 animated NPCs, fire,
ground items; ≈ 69k primitives).

Latest Sparky results (Chrome 153, Xvfb, NVIDIA GB10; not an owner/Mac/Edge acceptance):
every scene and model capture identical to the source PNGs (2073600/2073600 pixels);
fixture scenes 1749–1802 GPU-completed frames per 30 s (58.3–60.1 fps, GPU pass 0.5–0.9 ms);
workload 30 s: 1802 frames (59.85 fps), CPU build p50 7 / p95 12 ms, GPU p50 1.5 / p95 3.1 ms,
completion gap p95 30 ms, max 36 ms; workload 180 s: 10801 frames (59.85 fps), GPU p95 3.1 ms,
completion gap p95 28 ms, max 55 ms, 61 of 10800 gaps above 33.4 ms. These are Sparky
engineering measurements of genuine GPU-completed frames (RAF-capped at 60 Hz under Xvfb), not
the ≥ 60 fps contract proof on the owner's hardware.

## Contract notes for the shell

* Scene ids: the fixture names `lumbridge-castle-plaza`, `lumbridge-river-bridge`,
  `tutorial-starting-house`, `tutorial-survival-coast`, `lumbridge-windmill-route`
  (`FIXTURE_SCENE_IDS`, static captures) and region ids `region.osrs.<square>` /
  `regionSceneId(square)` for the streamed world (`PlayerView.region` uses the same form).
* `ScenePick` for scenery carries an extra `scenery` object (`objectId`, `type`, `spanX`,
  `spanY`) beside the contract fields; the contract itself is unchanged.
* `RendererConfig` needs no change. Optional additions that would help the shell but were **not**
  made to the shared file: a `zoom` derivation helper (provided here as
  `sourceZoomForViewportHeight`), a diagnostics accessor (provided as the
  `ClubscapeRendererHandle` extension), and two view fields the renderer can only consume if
  the shell forwards them: `WorldView.dynamicObjects` (mirror of the protocol
  `WorldSnapshot.dynamic_objects`: `{ id, objectId | sourceId, tile, instance, state, doorOpen,
  quarterTurns }`, typed here as `RendererDynamicObject`) for door states, and a running flag
  (`activity === "walking"` currently plays the walk sequence; run 824 needs the flag).
* Interface preview: call `framePlayerPreview({ width, height })` with the UI's
  `getUiPreviewBounds()` size (480×315 for the character creator) after `update(world)` so the
  worn gear matches, and hand the `ImageData` to `setUiPreview()` via a canvas of the same size.
  The renderer sways the model with the original 20 ms cycle, so re-request per animation frame
  while the interface is open.
