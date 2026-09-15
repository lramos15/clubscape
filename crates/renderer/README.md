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
| `scene::block` | `rl4.fn` output, `rd.az` | 64×64 world blocks assembled into a 104×104 scene around any chunk-aligned base; baked animated-scenery frame sets with the original sequence advance. |
| `scene::draw` (static replay) | – | `StaticModelCache`: projected triangles of static placements per camera signature, replayed while the camera holds still (identical stream; `tests/static_cache.rs`). |
| `scene::draw` (roofs, live layers) | `ez.or/ei` + `jz`, `cz.ch`, `eq/fk` item layers, `ez.ck` wall replace | Original roof removal (mode bits 1/2/4/8 incl. the camera-line Bresenham), the stock top-plane rule, ground-item layers at the three original draw points, door wall overrides. |
| `scene::minimap` | `client.bm/pv/ed/nq/wc/xx/ga`, `yz.as`, `yw.ed/es/dj/el`, `client.gq` | The original minimap raster: per-tile flags, tile sweep, shape-masked terrain fills, wall/door/diagonal marks, map-scene sprites, on the exact 2D primitives; 512×512 at 4 px/tile with the 48 px margin of the native HUD capture. |
| `anim` | `fx.rx/dn/jp`, `ou`, `et`/`em`, `rd.az` | Skeletal animation: per-label transform types 0/1/2/3/5 with the original int truncations, sequence timing (frame lengths, loop step, one-shot end), skeleton/frame decoding of the exported `anim/seq-<id>.bin` files. |
| `actor` | `pl.ag`, `lc.bd`, `op.jm` | NPC definitions (stand/walk/death/combat sequences, size, width/height scale), the approved penguin player body (NPC 2063, model 21547, 75/128) with the human→penguin label retarget table, per-slot equipment attachment + fit measurement, activity → player sequence selection. |
| `gpu` | – | 16×8 binned compute rasterizer (`raster.wgsl`) with identical arithmetic, coverage alpha for model-only surfaces, canvas blit, pollable readback, ring of timestamp readbacks. |
| `core` | – | `RendererCore`: assets, scene, camera, `WorldView` actors (skeletal port or baked packs), gear, dynamic objects/temporary objects/ground items/doors, roof mode, interface player preview, triangle stream, exact-coverage picking. |
| `wasm` (feature `web`) | – | `WasmRenderer` wasm-bindgen ABI consumed by `web/renderer`. |

## Building and testing

```bash
# native library + CPU tests on published inputs only (what CI runs; no GPU needed)
cargo test -p clubscape-renderer --release
# native GPU fidelity tests: a hardware wgpu adapter (Vulkan/Metal) is REQUIRED; without one
# every GPU test fails — a missing or software adapter is never a pass
cargo test -p clubscape-renderer --release --features gpu
# tests over reproducible local exports (ignored by default; missing files then fail):
#   export.py --profile blocks / --profile scenes-pinned / --profile npcs
cargo test -p clubscape-renderer --release --features gpu -- --include-ignored
# the precomputed per-pose gear fit table (assets/compiled/render/gear/pose-fits.json)
python3 tools/render-assets/export.py --profile pose-fits   # = cargo run --features tools --bin pose-fit-table
# lints as CI runs them
cargo clippy -p clubscape-renderer --release --features gpu,tools --all-targets -- -D warnings
cargo clippy -p clubscape-renderer --features web --target wasm32-unknown-unknown -- -D warnings
# browser artefact (also generates web/renderer/pkg and web/renderer/dist)
web/renderer/build.sh
```

Runtime inputs live in `assets/compiled/render` (see `tools/render-assets/README.md`). Tests read
published buffers through `tests/common/mod.rs`, which returns exactly the manifest-pinned
bytes: a raw file is used only when its SHA-256 equals the manifest entry, otherwise the
published gzip twin is inflated and its decompressed hash checked; textures are the manifest's
list, not the directory contents; a stale or newer unpublished local export can never override
a published input, and an input matching neither pin fails loudly (no silent fallback). Raw
scene buffers are published only as deterministic gzip twins, so a clean checkout runs every
scene and NPC case. Scene/block exports must carry their tile settings (`TSET`/`BSET`) — an
older export without them is rejected instead of silently drawing every roof (that defaulting
once let a stale raw scene hide the roof-removal failure). No test skips a mandatory case at
run time — suites over unpublished, reproducible exports (world blocks, pinned validation twins,
per-frame bakes) are `#[ignore]`d with the reproduction command and fail on missing or
unpinned files when run with `--include-ignored`. Pixel comparisons report counts, first
differences and channel errors, never whole buffers.

Tests (published inputs unless noted):

* `tests/fixtures.rs` – palette/table parity, tree ×4 yaws, **all 54 goblin/penguin capture
  frames from the published packs** (`models/npc-<id>.pack.bin`), the five scene fixtures,
  `RendererCore` actors/picking; ignored: local per-frame bakes equal the pack frames after the
  original `(int)` vertex truncation.
* `tests/robustness.rs` – truncated/corrupt buffers, out-of-range indices, missing textures and
  mismatched packs fail explicitly; unknown NPCs/sequences are reported; animation cadence equals
  the source frame lengths; NPC packs reproduce the approved frame captures through
  `RendererCore`; the skeletal port reproduces every baked pack frame vertex- and pixel-exactly;
  resize keeps the projection centre and coverage; picking respects bounds.
* `tests/actors.rs` – NPC definitions animate/pick/filter by instance, player action motions and
  gear fit, the `game.observer.v1` fields (explicit `running`/`movementTick` over the tick rule,
  action clocks stable across polls and reconnects, unbound/unknown actions reported, dynamic
  objects through `sourceId` only), live layers (items, fire, doors, roof modes), the stock
  top-plane rule, the interface preview projection.
* `tests/gear_fit.rs` – the bind-pose fit of all 10 worn models, and the **pose gate**: every
  item × 27 required sequences × frame after the per-pose contact fit must meet penetration ≤ 1
  and gap ≤ 2; the test FAILS while any frame is over (currently 20 of 3 220 item-frames over
  the gap target, see "Known deviations"); the published `gear/pose-fits.json` equals the live
  solve and its over-target count.
* `tests/hud_zoom.rs` – the `rl.cu` full-HUD zoom port against the original client's own values
  for 16 canvas sizes (`hud/zoom-table.json`), and the viewport-only 662/471/883 values.
* `tests/dynamic_layers.rs` – the 12 independent original dynamic-layer cases (see "Known
  deviations"), CPU and (`--features gpu`) GPU.
* `tests/minimap.rs` (native captures ignored by default) – map-scene assets, terrain fill,
  door marks, missing inputs; all 15 native minimap captures over the scene interior, and the
  icon list against the original `bu.aa` pass recorded per square.
* `tests/gpu.rs` (`--features gpu`) – GPU output equals the CPU port and the source PNGs for the
  five scenes and all 58 model captures (4 tree yaws + 54 pack frames); coverage alpha and the
  asynchronous readback. Logged GPU times are the completed frame's timestamp-query span or
  are labelled unavailable; wall time is reported separately.
* `tests/blocks.rs` (ignored by default) – scenes assembled from world blocks are pixel-identical
  to the original loader's direct export at all five fixture bases (animated scenery pinned to
  frame 0 on both sides); recentering around the player keeps the whole visible world; instance
  layouts (the M1 Death Office template: four turn-0 chunk mappings of square 12633) assemble
  only the declared chunks, tile-identical to the ordinary world inside and unloaded outside,
  with translated mappings shifting exactly and turned mappings rejected.

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
browser layer resolves `frame()` only after that callback fires (the callback wakes the pending
future directly; a 1 s watchdog only guards a silent device); a `requestAnimationFrame` tick is
not a completion. Frames may overlap: the next frame's CPU build runs while the previous frame's
GPU work completes (queue order keeps buffers safe), and each record still reports its own
submission's genuine completion. Timestamp readbacks use a ring of three buffers so overlapped
frames keep their measured spans.

## WASM ABI (`--features web`)

Generated by `wasm-bindgen --target web` into `web/renderer/pkg` (crate and CLI both pinned to
0.2.128). Exported class `WasmRenderer`:

| Member | Semantics |
| --- | --- |
| `new WasmRenderer(canvas, width, height, paletteBytes)` → `Promise<WasmRenderer>` | Requests a hardware WebGPU adapter/device through wgpu's browser backend and configures the canvas surface. Rejects when WebGPU is missing, the adapter is a software fallback, or the device request fails. No WebGL/image fallback exists. |
| `add_texture(bytes)` → texture id | Loads an exported texture (`textures/<id>.bin`). |
| `load_npc_pack(npcId, bytes)` | Loads a base lit model with its baked original animation frames. |
| `load_model(id, bytes)` | Loads a standalone lit model (developer model fixtures). |
| `load_scene(id, sceneBytes, modelPackBytes)` | Loads a fixture scene and its model pack; fails on pack mismatch or missing textures. |
| `load_block(square, blockBytes, packBytes)`, `has_block`, `unload_block` | World blocks (`blocks/<square>.bin[.gz]`, `square = x << 8 \| y`). |
| `WasmRenderer.squares_for_base(x, y)`, `WasmRenderer.base_for_tile(x, y)`, `needs_recenter(x, y, margin)` | The original scene geometry: squares the extended grid needs, the player-centred base `((tile >> 3) - 6) * 8`, and the 16-tile edge rule for rebuilding. |
| `assemble_scene(baseX, baseY, nowMs)` → missing squares | Assembles the scene from loaded blocks (unexported squares stay empty like unloaded map squares in the original) and restarts the scenery animation clock. With a `WorldView.instanceLayout` only the declared source chunks are copied, each to its destination chunk (per-tile corner heights, so a loaded chunk's edge is exact; a source chunk may repeat); every undeclared chunk stays unloaded — no terrain, scenery, object slots or minimap data — as the original `rl4.fn` template loader leaves it, and the scene id becomes `blocks@x,y#<template>`. `quarterTurns != 0` is rejected naming the mapping: block exports hold the original scene's lit placed geometry, which cannot be turned exactly without the raw location/terrain inputs. |
| `resize(width, height)` | Reconfigures the surface and projection centre. |
| `set_camera(x, height, y, pitch, yaw, zoom, far)` | World units / 16384 per turn; rejects zoom ≤ 0 or far < 50. |
| `update_world(json, nowMs)` | Shared `WorldView` JSON plus the optional `dynamicObjects` / `events` / `instanceLayout` extensions. Player and `npc` entities become scene actors with the original tile-span registration and facing from tile steps. Motion identity is explicit only, in this precedence: `animation` (bare or catalog `…sequence.<id>`); the `game.observer.v1` fields — `running` / `movementTick` (the movement actually executed this tick decides idle/walk/run; a final exhausted run step stays running, a later non-moving tick does not) and `action: ActorActionView` whose bound `animation` plays anchored on `cycleStartedAtTick` (600 ms per tick; the same `id` + cycle never restarts across polls or reconnects, a new cycle start re-anchors, `animation: null` keeps the stance and reports the explicit action id/activity/actionId as `motion unknown`, `action: null` ends it, `version != 1` is ignored with a diagnostic); the actor's latest `events[]` animation (until it ends / a newer one / movement / activity at rest); else the stance — stand 808, walk 819, run 824, by the original two-tiles-per-`tick` rule only for observers without `running` (the `run` setting decides in-between or without ticks). Actions/deaths without a source animation keep the stance and are reported as `motion unknown` (`unknown_motions()`, frame `skipped`); nothing is ever guessed from nearby scenery. `observer_v1()` tells whether the last view carried the observer fields. NPCs with a definition play its stand/walk locally like the original client. The player wears `player.equipment[].item.sourceId` models with the bind-pose contact fit plus the per-pose contact fit (see `load_pose_fit_table`). `temporary_object` entities (fires) animate from first appearance; `groundItems` draw the tile's top three stacks; `dynamicObjects[]` select doors through the shell's validated `sourceId` only (an entry without one is reported and not drawn; `objectId` strings are never parsed) and swap door walls by `doorOpen/quarterTurns`. Entities in another `instance` than the player are skipped. `instanceLayout` (template identity + `GenericInstanceChunkMapping` list) switches block assembly to the declared chunks only — see `assemble_scene`; `instance_layout_changed()` asks the shell to reassemble. `set_motion_fallback(true)` (developer fixtures only) enables the activity/adjacency table. |
| `load_pose_fit_table(json, bodyNpc)` → entries, `player_pose_fits()` → JSON | `gear/pose-fits.json` (`export.py --profile pose-fits`, `pose-fit-table` bin): the per-item × required-sequence × frame rigid contact shifts `PlayerBody::pose_fitted` would solve, recorded so drawing a frame costs no solve (rejected for another body NPC, other targets or stale frame counts; anything the table lacks is solved live). `player_pose_fits()` lists every drawn/previewed frame's fit per part: `{sequence, frame, itemId, slot, shift, direction, precomputed, penetration, gap, meetsTargets}` — `meetsTargets: false` entries are the exact remaining fit failures. |
| `squares_needed(baseX, baseY)`, `instance_layout_changed()` | Squares to fetch for a scene under the current instance layout (declared source squares only inside an instance), and whether the last `update_world` changed the layout. |
| `set_hide_local_player_body(bool)` | Developer fixture control only: draws no body for the local player (the controlled original dynamic-layer references carry none). Never a gameplay state. |
| `load_sequence(bytes)` → id | Exported skeletal sequence (`anim/seq-<id>.bin`). |
| `load_npc_definition(recordJson, baseBytes)` → npc id | `manifest.npc_definitions[i]` as JSON with its lit base model. |
| `load_player_body(penguinBase, widthScale, heightScale, nativeSequences, humanReference)` | Installs the approved player body (NPC 2063 base at 75/128, native sequences 5668/5666) and the human reference body used to retarget player-appearance sequences. |
| `load_equip_model(itemId, bytes)` | Worn item model (`models/item-<id>-equip.bin`). |
| `load_dynamic_object(objectId, type, orientation, plain, frames, frameLengths)` | Door/fire/state object variant with optional baked frames. |
| `load_ground_item(itemId, minQuantity, bytes)`, `register_ground_item_definition(itemId, price, stackable)` | Ground stack model for quantities ≥ `minQuantity`; the item's shop value and stackable flag (`manifest.ground_items[].price/.stackable`, `op.ef`/`op.ek`) drive the original pile selection (`lj.es`): each drop is appended to the tile deque and the greatest value (price × (quantity + 1) when stackable; first maximum wins) moves to the head, so the WorldView's oldest-first item order is replayed drop by drop; the pile draws the first two other distinct ids (in deque order) and then the top. |
| `set_top_plane_override(limit \| undefined)`, `set_instanced_map(bool)`, `set_hide_roofs(bool)` | `br`: the approved fixtures pinned the `dh` plane argument to 0; `undefined` applies the stock `cz.ch` rule (the player's plane when the hide-roofs preference is on or in instanced maps; otherwise all planes unless a roof-flagged tile of the player's plane lies on the camera→player line at pitch < 2480, then the player's plane). |
| `set_roof_mode(bits)`, `set_roof_context(hx, hy, dx, dy)` | Original roof removal (`ez.ny`): 1 player tile, 2 hovered tile, 4 walk destination, 8 camera line. 0 = stock. |
| `player_fit_report()` → JSON | Per worn item: bound human label, penguin label, `penetration` (≤ 1 target), `gap` (≤ 2 target), `anchorShift`/`shiftDirection` of the contact solve, `pcaBoxPenetration`, `designPenetration` (same measure on the human body), retarget scale — source units. |
| `scene_placement()` → JSON | `{baseX, baseY, sizeTiles: 104, blocks}` for HUD helpers. |
| `load_map_scenes(bytes)`, `has_map_scenes()` | Original map-scene sprites + tile-shape masks (`minimap/mapscenes.bin`). |
| `load_map_icons(bytes)` → count, `map_icon_sprites()` → JSON, `map_icon_pixels()` → RGBA | Original map-element minimap sprites (`minimap/mapicons.bin`: every element with `ps.ay` set, sprite from the client's own `ps.as(false)` fetch — 386 of 1268). JSON `[{element, width, height, offsetX, offsetY, maxWidth, maxHeight, category}]` in pixel order; alpha 0 exactly where the original `ym.af` blit skips value 0. Drawn by the HUD over the minimap surface with the original `client.zr`/`bo.as` rule (per icon `dx = (x << 7) + 64 − playerX`, `dy = (y << 7) + 64 − playerY` in source units, scaled by the minimap zoom, rotated by the map angle, sprite top-left at `centre + (dx', −dy') − size/2`; skipped beyond 80 units, clipped beyond 50). |
| `load_minimap_block(square, bytes)`, `has_minimap_block(square)` | Square sidecar (`minimap/blocks/<square>.bin`: wall placement configs, object-definition map fields); attaches to the block whenever it is loaded. |
| `minimap_surface()` → JSON, `minimap_pixels()` → `Uint8ClampedArray`, `minimap_mask()` → `Uint8Array` | The source minimap of the current scene on the player's plane (`client.bm(world, 512×512, 4.0, plane, 0, 0, 48, 48)`), redrawn only when the scene, plane or door states change (`revision`). JSON: `width, height, scale, marginX, marginY, baseX, baseY, plane, revision, complete, stats{terrainTiles, wallMarks, diagonalMarks, mapScenes, unresolved}, notes[], icons[{x, y, plane, element}], sourceIconMismatches` (icons: map-element ids of floor decorations as `bu.aa` collects them; `sourceIconMismatches` compares that list with the original pass the block sidecars recorded per plane — 0 = identical sets, n = differences named in `notes`, null = sidecars without a record). Pixels are RGBA alpha 255; the mask is 1 where the sweep drew map data. Rejects without a scene, without the map-scene asset, or with `complete: false` explained in `notes` when a square lacks its sidecar; never a blank or approximate map. Wall marks use the colours the approved native captures were drawn with (`0xEF0000` interactive, `0xF2E8F0` plain; the original jitters both per session). |
| `set_plane(plane)` | Plane for frames/minimap when no WorldView player is supplied (developer fixtures). |
| `frame_player_preview(optionsJson, nowMs)` → `Promise<Uint8ClampedArray \| undefined>` | Model-only interface preview of the current body + gear through the original type-6 component draw (interface 679:73 defaults from `manifest.model_widgets`: 480×315 parent layer, component centre, zoom 512, model zoom 450, offsetY2 175, content type 328 pitch 150 + 20 ms-cycle sway). Tightly packed RGBA with alpha 255 only where geometry was written; resolves after GPU completion and readback mapping. |
| `frame(nowMs)` → `Promise<FrameRecordJs>` | Builds, submits and presents one frame; resolves after GPU completion with `sequence`, `submitted_at_ms`, `completed_at_ms`, `draw_calls`, `primitives`, `cpu_encode_ms` (+ its breakdown `cpu_build_ms`, `cpu_pack_ms`, `cpu_upload_ms`, `cpu_present_ms`), `gpu_duration_ms`/`gpu_duration_known`, `entities_drawn`, `texture_fallbacks`, `skipped`. Rejects on device loss, uncaptured WebGPU errors or pipeline validation failures. |
| `frame_model_fixture(model, npc, sequence, frame, yaw, cameraY, cameraZ)` | Developer replay of an approved model capture (legacy draw, zoom 1024, background 0x303030). |
| `pick(x, y)` → JSON or `undefined` | Exact fill coverage of the last frame; every drawn thing carries the original tag layout (`x \| y << 7 \| plane << 14 \| layer << 16 \| id << 20`; layer 0 player, 1 npc, 2 scenery, 3 item pile). Actors: `{"kind":"entity","id":<WorldView id>,"tile"}`. Scenery: `{"kind":"tile","tile":<object origin tile>,"scenery":{"objectId","type":<placement type: walls 0–3, diagonal 9, objects 10/11, floor decoration 22>,"spanX","spanY"}}`, or `kind: "entity"` with the WorldView `object` entity id when the shell listed one on that footprint. Ground and item piles: `{"kind":"tile","tile"}` (the WorldView lists the items on that tile). |
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
* **Model pack** `scenes/<name>.models.bin`, `blocks/<square>.models.bin`: `CSMP` magic, count,
  then `(keyLen, dataLen, key, model chunk file)` entries in `MODL` key order. An entry may be a
  delta: `BASE` (index of an earlier full entry) + `MDHD` + `BNDC` + the changed `VRTX/VRTY/VRTZ`
  arrays — ground-contoured placements and baked animation frames of one lit model share its
  face data.
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

* **World block** `blocks/<square>.bin`: `BLHD` (square, rx, ry, origin x/y, size 64, planes,
  export base, margin, roof mode, min level, animated set count), `BFLG/BLNK/BOBC/BOBF/BHGT/BROF`
  per-tile arrays for the square, `BPNT` (10), `BTMD`, `BWAL` (12), `BWDC` (16), `BFDC` (9),
  `BOBJ` (17) placements keyed by `(plane, bx, by)` with square-relative units/spans, `BDYN`
  animated sets (`frames, frame_step, max_loops, total_cycles, plain, models[frames],
  lengths[frames]`), `MODL`. Model references `<= -2` denote animated set `-(ref) - 2`.

`assets/compiled/render/manifest.json` lists every buffer with SHA-256 and size; the adapter
refuses buffers whose hash differs.

* **Sequence** `anim/seq-<id>.bin`: `SEQH` (id, frame count, frame step, max loops, total
  cycles, left/right hand items, priority, reply mode, precedence, stretches), `SEQL` frame
  lengths (client cycles), `SEQF` frame refs (`archive << 16 | index`), optional `SEQI`
  interleave order, `SKEL` skeletons (`count, (type, labelCount, labels…)…`), `FRMT` frames
  (`ref, skeleton, transformCount, alpha, (transformIndex, dx, dy, dz)…`). Original `et`/`em`
  structures read from the pinned runtime, never re-derived.
* **Dynamic models** `models/dynamic/object-<id>-t<type>-r<rot>[-f<frame>].bin` (door/fire/state
  objects lit by the original `om.sg`) and `models/dynamic/item-<id>-q<min>.bin` (ground stacks
  by `op.aa`); `models/item-<id>-equip.bin` worn models (`op.jm`); `models/npc-<id>-base.bin`
  definition bases; `models/player-default-male-body.bin` the human retarget reference.
* **Tile settings** `TSET` (scenes) / `BSET` (blocks): `ez.vs` bytes (1 blocked, 2 bridge,
  4 roof, 8 force-lowest) used by roof removal and the stock top-plane rule.
* **Map scenes** `minimap/mapscenes.bin`: `MSHD` (sprite count, sprite group from the graphics
  defaults, palette length), `MSHP` the 16 tile-shape coverage masks (`client.gq`, `i64`),
  `MSPR` per sprite (original width/height, offset x/y, trimmed width/height, index count),
  `MSPL` shared palette, `MSPX` concatenated palette indices (0 transparent).
* **Minimap sidecar** `minimap/blocks/<square>.bin`: `MBHD` (square, rx, ry, origin, size,
  planes, wall count, definition count), `MWAL` (plane, bx, by, `fe.getConfig()` = type |
  rotation << 6) per wall, `MDEF` (object id, `mapSceneId`, size x, size y, `mapIconId`) for
  every object definition referenced by the square's walls, game objects and floor decorations.
  Wall type is needed because placement types 1 and 3 share one wall orientation; the tag's
  bit 19 (non-interactive) picks the wall colour and bits 20..51 the definition.
* `manifest.model_widgets`: original if3 model components decoded by `lw.ag` (interface 679
  component 73: 136×192 at originalY 32 in the 480×315 layer, model zoom 450, offsetY2 175,
  content type 328).

## Known deviations and open items

* Zone dynamic lists (`ZDYN`) are exported but empty in every fixture; the `si`/`dy` semantics
  are ported as read but untested against source pixels.
* Actor heights sample the original tile height field (bilinear, bridge-aware); the fixtures
  contain no live actors to compare against. Gear, action sequences and roof-removal *modes*
  (1/2/4/8) remain proven by geometry/pixel-change tests and browser captures only; ground
  piles, fires, door orientations, the stock top-plane selection and the plane-1 view are now
  matched against original renderings (previous bullet).
* The skeletal port reproduces all 54 baked goblin/penguin frames vertex- and pixel-exactly;
  Maya-skeletal sequences (`ou.bp >= 0`) are not supported (none are required for M1).
* Player action motions are the original human sequences retargeted onto the penguin labels
  (translation scale 161/196, bone table in `actor::HUMAN_TO_PENGUIN_LABELS`); this is a
  technical retarget of source timing/geometry, not a source capture of a penguin performing them.
* Gear fit (`actor::PlayerBody::assemble`, `tests/gear_fit.rs`): each worn model is scaled by the
  height ratio, placed at its retargeted design offset from the anchor label, then a contact
  solve translates the whole item along the slot's outward direction (weapon −X, shield +X,
  head up, amulet forward) by the smallest distance at which **penetration ≤ 1** — the deeper
  of (a) the deepest body vertex inside the item's minimum-volume oriented box (tighter of the
  principal-axis and model-axis boxes) and (b) the deepest item vertex inside the body mesh
  (winding number) — while the **gap ≤ 2** — the item↔body surface clearance. Item geometry is
  never edited and no faces are masked. Bind-pose values (source units): axe 0.99 / 0.11 (shift
  1.48), dagger 0.99 / 0.65 (4.48), sword 0.99 / 0.65 (3.66), pickaxe 1.00 / 0.26 (6.12),
  spear 0.99 / 0.65 (4.48), shortbow 0.99 / 0.31 (5.30), wooden shield 1.00 / 0.44 (6.63),
  square shield 1.00 / 0.66 (6.41), chef's hat 1.00 / 0.24 (0.70 up), brass necklace 1.00 / 0.63
  (21.4 forward, onto the chest front). The report also carries the principal-axis-only box
  figure so the box choice is visible (hat: 7.54 in that looser, tilted frame) and the same
  measure of the item on the human body it was designed for (hat 6.0, shields 10–11: the source
  designs overlap the body by construction).
* Per-pose contact fit (`actor::PlayerBody::pose_fitted`, `tests/gear_fit.rs` pose gate,
  `gear/pose-fits.json`): after each frame's retargeted pose, every worn item is measured
  against the posed body with the bind-pose box carried rigidly with the item (plus the
  embedded measure) and, where a body part swings into it deeper than 1 or it drifts further
  than 2 from the body, the item alone is translated rigidly for that frame: the shortest
  clearing shift over the slot axis, body axes, radial and item-box axes wins, a shift that
  also keeps contact is preferred within 1.5× + 4 units, the slot axis breaks near-ties, and a
  perpendicular slide / bounded lattice search restores contact when the clearing shift left
  the item floating. Items are fitted against the body only (no gear combination is special);
  item geometry and the source frame transforms are never edited. The same solve is recorded
  per item × sequence × frame in `gear/pose-fits.json` (deterministic, `export.py --profile
  pose-fits`) so the browser applies a recorded shift; `published_pose_fit_table_matches_the_live_solve`
  checks the table against the live solve. **Result over the 3 220 item-frames (10 items × 27
  required sequences): penetration ≤ 1 in every frame; 20 frames in 10 item × sequence entries
  still exceed gap 2 and the pose gate FAILS on them** — Bronze sq shield seq 829/12526 (5
  frames each, max gap 4.63), 625 (1, 3.08), 899 (1, 4.73), 711 (1, 3.09); Wooden shield
  829/12526 (2 each, 3.04); Bronze pickaxe 836 (1, 2.12), 625 (1, 2.78), 733 (1, 9.21). In those
  frames no rigid translation within the search bounds meets both targets: the box measure
  (a slab around a shield / a long handle) clears the round body only where the item's surface
  is already more than 2 units away. Largest shifts applied: sq shield 57 (seq 426 — bow
  motion, not wearable with a shield), 48.5 wooden shield (426), 24 pickaxe (733), 17 necklace
  (625), 7 hat (625/621); the source's own human motions drive the same items through the
  human body (design penetration shield 10–11, hat 6–7, pickaxe 5). Reported, not waived.
* Dynamic layers against the independent original references (`tests/dynamic_layers.rs`,
  `assets/reference/osrs240/m1-dynamic`, 12 controlled offline original-client renderings at
  the native **full-HUD zoom 410** — a second original projection beside the frozen
  viewport-only fixtures' 662 at the same 1080 px height; each is applied only to its own
  cases, never one onto the other). Measured with the approved `native_scene_model` profile
  over every pixel outside the three native HUD container rectangles (which the original paints
  over the scene; they are counted and reported, not masked scene geometry): door closed/open
  (object 9398, orientations 0/1), ground piles (single coin, 10 000-coin stack, four items →
  top three by the `lj.es` value/deque rule), fire frames 0 and 3 (sequence 475, 19 cycles),
  roofs outside/inside/hidden, plane-1 view — **8 of 11 rendered cases pixel-identical**
  (1 863 553 / 1 863 553 scene pixels, interior max 0, band 0). The three Tutorial plane-0
  cases (door closed/open, roofs hidden) differ in 95 scene pixels, all inside the screen boxes
  of three animated flames (object 24969 at 3095,3102; object 196 torches at 3096,3105 and
  3096,3110) whose start phase the original draws from `Math.random()` per placement and the
  case records do not carry; at the fixture-export phase they are **unpassed with that cause**.
  `flame_phases_identified_from_blocks` (ignored, block exports) steps each placement through
  its five baked frames independently, reads the phases the references show (4, 0, 2) and then
  matches all three frames pixel-exactly (`set_scenery_phase`). The stock normal-camera
  selector `cz.ch` now honours the original hide-roofs preference first (`set_hide_roofs`) and
  reproduces every recorded selector value (3 outside, 0 inside/hidden, 1 on plane 1).
  `lumbridge-minimap-mapped-chunk` (controlled `rl4.fn` template mapping of source chunk
  402,402 turned 2 quarter turns onto local chunk 6,6) stays **unsupported**: instance chunk
  assembly exists for turn-0 mappings (the M1 Death Office template), but a turned chunk needs
  the terrain re-lit and every model re-lit at the turned orientation from raw inputs the block
  exports do not carry, so it is rejected explicitly rather than approximated. The same eight
  phase-independent cases also pass through the wgpu compute rasterizer on the GB10
  (`dynamic_layer_cases_match_on_the_gpu`: GPU = CPU = source, 0 differing pixels). Chrome 153
  (dev scenarios `source-*`: full-HUD zoom from `fullHudZoomForViewport`, no local player body
  like the source input): `tutorial-roofs-outside` and `tutorial-roofs-inside` are identical
  over all 1 863 553 scene pixels; door closed/open and roofs hidden differ in the same 95
  flame-box pixels as the CPU/GPU tests, 0 elsewhere.
* Minimap (`scene::minimap`, `tests/minimap.rs`): all 15 native minimap captures (5 scenes ×
  planes 0–2) are reproduced pixel-exactly over the scene interior (tiles 5..98) from streamed
  blocks — terrain shapes/colours, bridge tiles, wall/door/diagonal marks, map-scene sprites.
  The outer 5-tile band of a 104×104 scene differs (up to 14 207 of 262 144 pixels on plane 0,
  0 on most upper planes): the original loader blends floor colours there from inputs that
  depend on the scene base, while blocks carry colours blended once with full neighbours. The
  same band exists in the assembled 3D scene (never within the original's 16-tile recenter
  margin of the player, rarely visible at draw distance 25). Exact band colours would need the
  raw underlay/overlay ids blended at assembly, which the block export does not yet carry.
  Map-element icons: the surface's icon list equals the original `bu.aa` pass recorded per
  square on all 15 native cases (`sourceIconMismatches: 0`), and the original sprites are
  exported (`minimap/mapicons.bin`, 386 elements) with the `bo.as` placement rule for the HUD;
  the sprites are not composited into the surface because the original draws them per frame
  around the player, so there is no native icon-layer image to compare a composite against.
* Motion identity depends on the backend filling `Player.animation`/`Entity.animation` (or the
  shell forwarding animation events); until then actions render the stance with an explicit
  `motion unknown` diagnostic. Running follows the original tiles-per-tick rule from
  `WorldView.tick` plus the `run` setting. Door states need the protocol `dynamic_objects`
  forwarded as the optional `dynamicObjects` extension (see `web/renderer/README.md`).
* Animated scenery uses native frame bakes (`dy.vn` per source frame) with the original sequence
  advance ported (`rd.az`: frame lengths, loop step, one-shot reset to the plain model). Start
  phases are per-placement pseudo-random like the original `Math.random` seeds, so they are not
  pixel-comparable to a capture; the block tests pin frame 0 on both sides.
* World block size: the 61 M1 squares total ≈ 75 MB gzip (the sea square 12078 alone ≈ 10 MB
  because its wave scenery has 124 baked frames of 3.2k-vertex models). Blocks stay out of git
  (`export.py --profile blocks` reproduces them; hashes are in the manifest).
