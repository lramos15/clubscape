# clubscape-renderer

Source-faithful ClubScape scene/model renderer. The crate is an exact port of the pinned original
client's software rendering path (`injected-client-1.12.38`, revision 240, cache 2695) to Rust with
wrapping 32-bit integer and IEEE `f32` semantics, plus a WebGPU compute rasterizer that evaluates
the same integer fills per pixel. Every approved reference-pack capture (5 world scenes, 4 tree
yaws, 54 goblin/penguin animation frames) is reproduced **pixel-identically** on the CPU port, on
the native wgpu backend (NVIDIA GB10 / Vulkan) and inside windowed Chrome 153 via WebGPU.

This is a renderer only: it draws the supplied scene buffers and the authoritative `WorldView`
handed over by the shell. It contains no game rules, no hit points, no interaction results.

## Layout

| Module | Original | Purpose |
| --- | --- | --- |
| `chunk` | – | Little-endian tagged chunk container used by every exported buffer. |
| `tables` | `net.runelite.api.Perspective`, `fh`/`gb` | 2048- and 16384-unit sine/cosine tables, reciprocal table; hash-checked against the exporter. |
| `palette` | `fh.ae`/`fh.bk` | 65536-entry HSL→RGB palette at brightness 0.8 (builder matches the export byte for byte). |
| `texture` | `ec`/`fu` | 128×128 palette-indexed textures, `ag` opaque flag, animation scroll. |
| `model` | `fx`, `ModelData` | Lit float-vertex model, cylinder (`fx.et`) and sphere (`fx.bd`) bounds, model packs. |
| `raster::software` | `ft.ao/al/aj/ay` + scanlines `jm/px/bq/bf` | Exact CPU fills (4-px Gouraud banding, alpha-254 shift copy, per-8-px perspective texture spans, `fq.af` missing-texture fallback). |
| `model_draw` | `fx.be`/`fx.xm`/`ja`/`si.av`/`kx`/`bz`/`gb`/`cb` | Legacy (2048 units/turn) and live-scene (16384 units/turn) projection, depth buckets, priority sort, near-plane clipping. |
| `scene` | `ez.dh/bh/ei/ee`, `eu`, `fv.zw`, `ei.pv`, `client.cw` | Scene buffers, visible-tile table, tile paint/model projection, two-pass spiral traversal and per-tile draw queue. |
| `gpu` | – | 16×8 binned compute rasterizer (`raster.wgsl`) with identical arithmetic, canvas blit, readback, timestamp queries. |
| `core` | – | `RendererCore`: assets, scene, camera, `WorldView` actors (baked original frames), triangle stream, exact-coverage picking. |
| `wasm` (feature `web`) | – | `WasmRenderer` wasm-bindgen ABI consumed by `web/renderer`. |

## Building and testing

```bash
# native library + CPU tests (no GPU needed)
cargo test -p clubscape-renderer --release
# native GPU differential tests (wgpu Vulkan/Metal adapter required)
cargo test -p clubscape-renderer --release --features gpu
# lints as CI runs them
cargo clippy -p clubscape-renderer --release --features gpu --all-targets -- -D warnings
cargo clippy -p clubscape-renderer --features web --target wasm32-unknown-unknown -- -D warnings
# browser artefact (also generates web/renderer/pkg and web/renderer/dist)
web/renderer/build.sh
```

Runtime inputs live in `assets/compiled/render` (see `tools/render-assets/README.md`). The raw
scene buffers are published only as deterministic gzip twins; restore them before running the
scene tests with `python3 tools/render-assets/export.py --profile unpack`. Tests that need an
absent local export skip themselves and say so.

Tests:

* `tests/fixtures.rs` – palette/table parity, tree ×4 yaws, 54 baked NPC frames (when baked dumps
  are present), the five scene fixtures, `RendererCore` actors/animation/picking.
* `tests/robustness.rs` – truncated/corrupt buffers, out-of-range indices, missing textures and
  mismatched packs fail explicitly; unknown NPCs/sequences are reported; animation cadence equals
  the source frame lengths; NPC packs reproduce the approved frame captures through
  `RendererCore`; resize keeps the projection centre and coverage; picking respects bounds.
* `tests/gpu.rs` – GPU output equals the CPU port and the source PNGs for every fixture.

## Coordinate and unit conventions

* World units: 128 per tile. `Camera { x, height, y }` uses **source X/Y as the horizontal
  axes** and `height` as the negative-up vertical (ground under the Tutorial house is ≈ −1360;
  the approved camera is −2360). Internally `y` maps to the client's Z axis.
* Scene camera angles: 16384 units per turn (`pitch` 2048 = source fixture). Model fixtures use
  the legacy 2048-unit draw (`fx.be`); the two are never mixed.
* Zoom is the client's viewport-derived value: `Camera::source_zoom_for_height(h)` → 662 at
  1080 px, 883 at 1440 px, 471 at 768 px. The renderer never rescales on its own.
* Draw distance 25 tiles is baked into the scene visibility tables; far clip 32768 is the
  fixture projection setting (`fq.ab`), near plane 50.
* Colors: the 65536-entry palette at brightness 0.8. Textured fills carry `0xFF` in the unused
  high byte exactly like the original pixel array; displays ignore it.

## Frame records

`GpuRasterizer::render` returns a `GpuFrame` whose `is_complete()` flips from the queue's
`on_submitted_work_done` callback; `gpu_duration_ns()` is the timestamp-query span of the fill
pass when `TIMESTAMP_QUERY` is supported (labelled unknown otherwise, never estimated). The
browser layer resolves `frame()` only after that callback fires; a `requestAnimationFrame` tick
is not a completion.

## WASM ABI (`--features web`)

Generated by `wasm-bindgen --target web` into `web/renderer/pkg` (crate and CLI both pinned to
0.2.128). Exported class `WasmRenderer`:

| Member | Semantics |
| --- | --- |
| `new WasmRenderer(canvas, width, height, paletteBytes)` → `Promise<WasmRenderer>` | Requests a hardware WebGPU adapter/device through wgpu's browser backend and configures the canvas surface. Rejects when WebGPU is missing, the adapter is a software fallback, or the device request fails. No WebGL/image fallback exists. |
| `add_texture(bytes)` → texture id | Loads an exported texture (`textures/<id>.bin`). |
| `load_npc_pack(npcId, bytes)` | Loads a base lit model with its baked original animation frames. |
| `load_model(id, bytes)` | Loads a standalone lit model (developer model fixtures). |
| `load_scene(id, sceneBytes, modelPackBytes)` | Loads a scene and its model pack; fails on pack mismatch or missing textures. |
| `resize(width, height)` | Reconfigures the surface and projection centre. |
| `set_camera(x, height, y, pitch, yaw, zoom, far)` | World units / 16384 per turn; rejects zoom ≤ 0 or far < 50. |
| `update_world(json, nowMs)` | Shared `WorldView` JSON; player and `npc`/`player` entities with a `sourceId` become scene actors on their tiles with the original tile-span registration, facing from tile steps and baked animations selected by the numeric `animation` string. |
| `frame(nowMs)` → `Promise<FrameRecordJs>` | Builds, submits and presents one frame; resolves after GPU completion with `sequence`, `submitted_at_ms`, `completed_at_ms`, `draw_calls`, `primitives`, `cpu_encode_ms`, `gpu_duration_ms`/`gpu_duration_known`, `entities_drawn`, `texture_fallbacks`, `skipped`. Rejects on device loss, uncaptured WebGPU errors or pipeline validation failures. |
| `frame_model_fixture(model, npc, sequence, frame, yaw, cameraY, cameraZ)` | Developer replay of an approved model capture (legacy draw, zoom 1024, background 0x303030). |
| `pick(x, y)` → JSON or `undefined` | `{"kind":"tile","tile":{x,y,plane}}` or `{"kind":"entity","id","tile"}` from exact fill coverage of the last frame; `id` is the scene object hash or the actor hash. |
| `adapter_info()`, `timestamps_supported()`, `device_epoch()`, `device_lost_reason()`, `scene_id()`, `last_frame_triangles()` | Diagnostics. |

The TypeScript adapter in `web/renderer/src/index.ts` maps this onto `CreateRenderer` /
`RendererHandle` from `web/shared/contracts.ts`.

## Exported buffer formats

Every file is a sequence of chunks: 4-byte ASCII tag, little-endian `u32` length, payload.
Integer chunks are little-endian `i32`; float chunks are IEEE `f32`; short chunks `i16`.

* **Tables** `tables.bin`: `SIN2`/`COS2` (2048), `SN14`/`CS14` (16384), `SNF2`/`CSF2`,
  `SF14`/`CF14` (float variants), `RCP1` (65536/n), `CSRC` (source note).
* **Palette** `palette.bin`: `BRGT` (brightness ×1000), `PLTE` 65536 RGB ints.
* **Texture** `textures/<id>.bin`: `TXHD` header (id, size, opaque flag, average RGB, animation
  direction/speed), `TXPX` 128×128 texel palette-RGB ints (0 = transparent).
* **Model** `models/*.bin` and entries in packs: `MDHD` (vertex/face/textured-face counts,
  priority, transparency, flags), `VRTX/VRTY/VRTZ` (`f32`), `FIDA/FIDB/FIDC`, `FCLA/FCLB/FCLC`
  (lit HSL colors, `-1` flat, `-2` hidden), `FTEX`, `FTXC`, `FPRI`, `FALP`, `FBIA`, `TXPI/TXMI/TXNI`,
  `VGRP/VGR2/FGRP/FGR2`, `BNDC` cylinder bounds.
* **Model pack** `scenes/<name>.models.bin`: `CSMP` magic, count, then `(keyLen, dataLen, key,
  model chunk file)` entries in scene `MODL` key order.
* **NPC pack** `models/npc-<id>.pack.bin`: base model chunks + `ANIM` (sequence id, frame count,
  frame lengths in 20 ms client cycles)*, `FRMS` `i16` xyz per vertex per frame (source scale
  applied, e.g. 75/128 for the penguin), `FBND` per-frame cylinder bounds + sphere radius,
  `SCAL` width/height scale.
* **Scene** `scenes/<name>.bin`: `SCHD` header (base x/y, planes, extended 184×184 grid, offset
  40), `NAME`, `FLAG` per-tile flag words (`scene::flag`), `HGHT` heights, `LINK` bridge/roof
  links, `ROOF`, `PANT` paints (8 ints), `TMOD` shaped tile models, `WALL` (10), `WDEC` (14),
  `FDEC` (7), `GOBJ` game objects (16), `OBJC`/`OBJF` per-tile object slots, `ZDYN` zone dynamic
  lists, `MODL` distinct model keys. Tile index = `plane << 16 | x << 8 | y`; bridge-moved tiles
  live at plane 3 without the `EXISTS` bit.

`assets/compiled/render/manifest.json` lists every buffer with SHA-256 and size; the adapter
refuses buffers whose hash differs.

## Known deviations and open items

* Roof hiding (`roof_mode ≠ 0`) and item layers are not exported or drawn; the approved fixtures
  contain neither.
* Zone dynamic lists (`ZDYN`) are exported but empty in every fixture; the `si`/`dy` semantics
  are ported as read but untested against source pixels.
* Actor heights sample the original tile height field (bilinear, bridge-aware); the fixtures
  contain no live actors to compare against.
* Animation playback uses native original frame bakes (source timing and geometry preserved);
  the skeleton/frame decoder itself is not ported, so only exported sequences play. Required
  player action animations beyond the approved goblin/penguin sets and penguin equipment fitting
  are not implemented (see the task report).
* Per-placement contoured models are stored individually (~5k per scene); the gzip twins keep
  the published size at ≈ 2.7 MB per scene.
