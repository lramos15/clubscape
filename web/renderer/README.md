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
plus `diagnostics()` (adapter name, timestamp support, device epoch, manifest SHA, loaded asset
ids/hashes, rendered frame count, last frame, device-loss reason) and the developer-only
`frameModelFixture()`. `createRendererContract` is the plain `CreateRenderer` signature.

## Behaviour

* Requires `navigator.gpu`; rejects with the exact cause when WebGPU, a hardware adapter or the
  device is unavailable. There is no WebGL, 2D-canvas or image fallback.
* Fetches `manifest.json`, checks `kind`/`schema_version` and that
  `approved_reference_pack_sha256 === config.sourcePackSha256`, then fetches palette, the 33
  textures and the NPC animation packs, verifying each SHA-256 against the manifest.
  `loadScene(id)` fetches the scene buffer and model pack (gzip twins, decompressed with
  `DecompressionStream`, both hashes verified). Any mismatch throws.
* `camera()` takes `RenderCamera` in world units (128 per tile; `x`/`y` horizontal source axes,
  `height` negative-up, 16384 angle units per turn). Pass `zoom = sourceZoomForViewportHeight(h)`
  to reproduce the client's viewport curve (662 at 1080 px); the renderer never rescales itself.
* `update(world)` serialises the `WorldView`; the player and `npc`/`player` entities with a
  numeric `sourceId` are drawn on their tiles using baked original animations selected by the
  `animation` string (goblin 6181/6180, penguin 5668/5666). Unknown NPC ids or sequences are
  reported through `options.onDiagnostic`, never invented.
* `frame(nowMs)` returns `null` while a frame is in flight or before a scene is loaded, otherwise a
  `RenderFrame` whose `completedAtMs` is taken after the WebGPU queue's submitted-work-done
  signal (`gpuDurationMs` from timestamp queries when supported; omitted otherwise). Uncaptured
  WebGPU errors, pipeline validation failures and device loss reject the promise.
* `pick(x, y)` replays the last frame's exact fill coverage and returns the topmost tile or
  entity (`id` is the scene object hash or the entity id hash). It never mutates state.
* `resize(w, h)` resizes the canvas, surface and projection; call `camera()` again with the new
  zoom.

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
drive the adapter with the approved fixture cameras (`?scene=<id>&w=&h=&actors=1`, or
`?mode=model` for capture replays) and expose `window.__clubscapeBenchmarkV1` (RenderSnapshot
protocol) and `window.__clubscapeDev`. `dev/capture.ts` launches the pinned windowed Chrome for
Testing with the sandbox on and the `sparky-vulkan-x11` graphics arguments from
`docs/machines/sparky.md` (no `--no-sandbox`, no software renderer), screenshots the visible
canvas element for the five scenes, replays the 58 approved model captures, checks
1024×768 / 1280×720 / 2560×1440, a live resize, actors and picks, and writes `report.json` with
GPU-completed frame statistics. Latest Sparky result: every scene and model capture identical to
the source PNGs (2073600/2073600 pixels), ≈ 59–60 GPU-completed fps at 1920×1080, GPU pass
0.5–0.9 ms, submit→complete p95 ≤ 9 ms.

## Contract notes for the shell

* Scene ids are the exported names: `lumbridge-castle-plaza`, `lumbridge-river-bridge`,
  `tutorial-starting-house`, `tutorial-survival-coast`, `lumbridge-windmill-route`
  (`FIXTURE_SCENE_IDS`). The manifest is the source of truth for further regions.
* `RendererConfig` needs no change. Two optional additions would help the shell but are **not**
  required and were not made to the shared file: a `zoom` derivation helper (provided here as
  `sourceZoomForViewportHeight`) and a diagnostics accessor on `RendererHandle` (provided as the
  `ClubscapeRendererHandle` extension).
