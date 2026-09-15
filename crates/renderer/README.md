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
* `tests/gear_fit.rs` – bind-pose attachment of the 10 equippable worn models, the source
  legality of item × sequence combinations (`lc.bd` hand overrides), the **pose gate** — every
  legal item × required-sequence × frame after the attachment-preserving fit must meet
  penetration ≤ 1, surface gap ≤ 2 **and attachment gap ≤ 2** — which is currently **UNMET**
  and therefore `#[ignore]`d with the exact count in its reason (671 of 1 587 legal item-frames;
  `--include-ignored` runs it and it fails), plus the non-ignored proofs that the published
  failure record `gear/pose-fit-failures.json` is exactly the live measurement and that
  `gear/pose-fits.json` (schema 2) equals the live solve. See "Known deviations".
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
| `update_world(json, nowMs)` | Shared `WorldView` JSON plus the optional `dynamicObjects` / `events` / `instanceLayout` extensions. Player and `npc` entities become scene actors with the original tile-span registration and facing from tile steps. Motion identity is explicit only, in this precedence: `animation` (bare or catalog `…sequence.<id>`); the `game.observer.v1` fields — `running` / `movementTick` (the movement actually executed this tick decides idle/walk/run; a final exhausted run step stays running, a later non-moving tick does not) and `action: ActorActionView` whose bound `animation` plays anchored on `cycleStartedAtTick` (600 ms per tick; the same `id` + cycle never restarts across polls or reconnects, a new cycle start re-anchors, `animation: null` keeps the stance and reports the explicit action id/activity/actionId as `motion unknown`, `action: null` ends it, `version != 1` is ignored with a diagnostic; every observer tick — `tick`, `movementTick`, `cycleStartedAtTick` — is decoded losslessly as a decimal `u64` (`core::parse_observer_tick`) and a malformed one on an observer view is an `update_world` error before any state changes, never a silent clock reset; a cycle start dated after the view's tick is reported and anchored at the update); the actor's latest `events[]` animation (until it ends / a newer one / movement / activity at rest); else the stance — stand 808, walk 819, run 824, by the original two-tiles-per-`tick` rule only for observers without `running` (the `run` setting decides in-between or without ticks). Actions/deaths without a source animation keep the stance and are reported as `motion unknown` (`unknown_motions()`, frame `skipped`); nothing is ever guessed from nearby scenery. `observer_v1()` tells whether the last view carried the observer fields. NPCs with a definition play its stand/walk locally like the original client. The player wears `player.equipment[].item.sourceId` models with the sequence's `lc.bd` hand overrides applied (`actor::effective_gear`) and the attachment-preserving bind + per-pose fits (see `load_pose_fit_table`, "Known deviations"). `temporary_object` entities (fires) animate from first appearance; `groundItems` draw the tile's top three stacks; `dynamicObjects[]` select doors through the shell's validated `sourceId` only (an entry without one is reported and not drawn; `objectId` strings are never parsed) and swap door walls by `doorOpen/quarterTurns`. Entities in another `instance` than the player are skipped. `instanceLayout` (template identity + `GenericInstanceChunkMapping` list) switches block assembly to the declared chunks only — see `assemble_scene`; `instance_layout_changed()` asks the shell to reassemble. `set_motion_fallback(true)` (developer fixtures only) enables the activity/adjacency table. |
| `load_pose_fit_table(json, bodyNpc)` → entries, `player_pose_fits()` → JSON | `gear/pose-fits.json` schema 2 (`export.py --profile pose-fits`, `pose-fit-table` bin): the per-item × drawn-sequence × frame rigid attachment fits (rotation about the grip + translation) `PlayerBody::pose_fitted` would solve, recorded so drawing a frame costs no solve (rejected for another body NPC, other targets, schema 1 or stale frame counts; anything the table lacks is solved live). `player_pose_fits()` lists every drawn/previewed frame's fit per part: `{sequence, frame, itemId, slot, shift, direction, rotation, precomputed, penetration, gap, attachmentGap, anchorClearance, meetsTargets}` — `meetsTargets: false` entries are the exact remaining fit failures. |
| `set_hand_override_kits(kits)` | Kit ids the source cache has (manifest `sequence_hand_overrides.values[kind=kit, kit_exists]`) for the `lc.bd` decode: a sequence whose hand item names a kit the cache lacks draws nothing in that slot. Applied automatically by the adapter. |
| `unbound_actions()` → JSON | `[{actorId, id, activity, actionId, recipeId, styleId, spellId}]`: the observer-v1 actions the last `update_world` reported with `animation: null` — exact identities awaiting the backend binding (also in `unknown_motions()` prose). |
| `minimap_icon_placement(tileX, tileY, playerFineX, playerFineY, scale, minimapAngle, widgetW, widgetH, spriteMaxW, spriteMaxH, spriteOffsetX, spriteOffsetY)` → JSON / `undefined`, `minimap_icon_placements(playerTileX, playerTileY, scale, widgetW, widgetH)` → JSON | Exact `client.zr` → `bo.as` marker placement in **minimap pixels** (see `scene::minimap::icon_placement`): `dx = ((tile << 7) + 64 − playerFine) * scale` truncated (stock scale `0.03125`: 4 px per tile), cut off at `dx² + dy² > 6400` (80 px = 20 tiles), rotated with the 16384-step 16.16 tables (`>> 16`), canvas top-left `(W/2 + rx − maxWidth/2, H/2 − ry − maxHeight/2)`, mask-clipped (`ym.bc`, no sprite offsets) beyond 2500 px², plain (`aap.ro`, sprite offsets added) inside. The bulk form places the current surface's icons with the loaded sprites and the camera yaw. |
| `squares_needed(baseX, baseY)`, `instance_layout_changed()` | Squares to fetch for a scene under the current instance layout (declared source squares only inside an instance), and whether the last `update_world` changed the layout. |
| `set_hide_local_player_body(bool)` | Developer fixture control only: draws no body for the local player (the controlled original dynamic-layer references carry none). Never a gameplay state. |
| `set_scenery_phase(plane, x, y, objectId, frame, cycle)` → count, `set_scenery_clock_override(cycles)` | Developer fixture controls only: replay an original capture's recorded animated-scenery controller state (`dy.ac` frame + cycle within the frame at scene start) for the instances of an object on a tile, and freeze the scenery animation clock at `cycles` client cycles since scene start (negative = real time) so a frame renders at exactly the cycle a capture was drawn at. |
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
* Gear attachment (`actor::PlayerBody::assemble_parts`, `tests/gear_fit.rs`): each worn model
  is scaled by the height ratio (161/196) and placed so that its **grip** — the item vertex
  nearest the human anchor label's centroid (the hilt in the hand, the strap on the arm, the
  hat's underside) — keeps its design offset from the anchor part's outer surface (held items:
  the ray from the label centroid along the outward axis through the part's own triangles, i.e.
  the palm/flipper face at the grip height; worn items: the label's extreme along the outward
  axis, the top of the skull / front of the chest). The outward axis follows the item's human
  label (right-hand labels −X, left-hand +X, head up, neck/chest forward), so a sequence hand
  item put into the "wrong" slot by `lc.bd` still faces the right way. The **attachment fit**
  then turns the item about its grip (≤ 90° about 13 lattice axes, refined by bisection) and
  translates it by at most **2 source units in total** across the bind fit and the per-frame
  fit (the bind translation is carried into each pose by the anchor bone's rotation and counted).
  Three measures are gated, none relaxed: **penetration ≤ 1** (the deeper of the deepest body
  vertex inside the item's minimum-volume oriented box fixed at the bind pose and carried with
  the item, and the deepest item vertex inside the body mesh), **gap ≤ 2** (item↔body surface
  clearance: a surface metric only — touching anywhere), and **attachment gap ≤ 2** (the grip's
  distance from where the posed anchor bone carries the retargeted design grip — the metric
  that says the item is held/worn where the source attaches it, which surface contact alone
  cannot). The anchor-part clearance is reported beside them. Item geometry is never edited, no
  faces are masked, no gear combination gets its own avatar.
* Which item × sequence combinations exist follows the source, not a hand-picked list
  (`actor::fit_legality`, manifest `sequence_hand_overrides`): `lc.bd` puts a sequence's
  `leftHandItem`/`rightHandItem` (`ou.by`/`ou.bq`, value − 512 + 2048 → item ≥ 2048, kit
  256..2047) into the shield/weapon slot while it plays — value 0 names kit 1280, which the cache
  does not have, so the slot draws nothing. Death 836 and 829/12526/827/711/897/896/899 hide
  both hands; 625 draws the pickaxe in both hands; 879 draws the axe through the shield slot with
  the weapon hidden; 621/733/898 draw the net/tinderbox/hammer (exported as `role:
  sequence_hand_item`, not wearable) with the other hand hidden. Classes over the 27 required
  sequences: `worn` 1 204 item-frames, `override` 95, `combat_binding_pending` 288 (a worn
  weapon/shield during 386/390/422/423/426 — kept as a superset until the backend publishes the
  weapon-category → animation binding), `hidden` 2 281, `not_equippable` 318 (not drawn, no fit).
  The runtime applies the same overrides to the drawn player and the interface preview
  (`core::player_frame_model`, `effective_gear`), and reports a hidden slot in
  `unknown_motions()`.
* **Required sequence set grown to 39** (confirmed additions in
  `research/interface-contracts/animation-requirements.json`, source metadata
  `animation-authority/inputs.json` sha `43fafa67…`): milking 2305 (native hand words left 6244 →
  stool 5732, right 2437 → bucket 1925), the five ordinary Home Teleport actor phases
  4847/4850/4853/4855/4857 (ticks 0/6/12/16/21 of the unchanged 24-tick channel — a qualified
  alignment, not an observed server packet; 4847 right word 10214 → stick 9702, every other
  hand hidden; the neighbouring 4848/4849/4854/4856/4858 are graphics, not actor clips), and the
  qualified legal combat styles 395 axe hack, 400 blunt spike, 401 blunt pound, 428 spear spike,
  429 spear lunge, 440 sweep. All twelve are exported with their original skeletons/frames and
  hand-model overrides (frame counts equal the native metadata) and play in Chrome with the
  right hand closure. Combat legality now follows the qualified weapon → style table
  (`actor::COMBAT_STYLES`) instead of a pending superset: a worn weapon is drawn only during the
  styles it plays, a shield only with one-handed styles or unarmed; other combinations are
  `illegal_style` (never occur, not drawn). Legality classes over 39 sequences: `worn` 1 801,
  `override` 233, `illegal_style` 497, `hidden` 4 863, `not_equippable` 942 item-frames → 2 034
  legal item-frames gated.
* **Pose gate result: UNMET — 931 of 2 034 legal item-frames** (926 penetration > 1, 8 gap > 2,
  0 attachment > 2): brass necklace 519, wooden shield 97, square shield 79, milking stool 55,
  pickaxe 36, axe 30, sword 23, dagger 17, stick 14, net 37, bow 9, spear 7, tinderbox 2, hat 1.
  Provenance: 67 are decided by the carried-box half alone (square shield 60, wooden shield 5,
  pickaxe 1, hat 1); 864 have item geometry inside the penguin mesh; 564 exceed the source
  design's own same-pose overlap by more than the target (necklace 496, wooden shield 19, net 37,
  stool 7, stick 3, sword 2). The previous 27-sequence figures for reference: 671 of 1 587 —
  666
  penetration > 1 (max 18.7, wooden shield seq 819/820), 8 gap > 2 (max 6.0, net 621), 0
  attachment > 2 (bounded by construction). Per item: brass necklace 321 (every frame: a chain
  designed for a neck ~15 units across is embedded 8–13 deep in the penguin's ~50-unit-deep body
  when attached at the neck — floating 24 units in front of the chest, as the previous
  surface-only fit placed it, is not attached); wooden shield 85 and square shield 73 (the strap
  carries the flipper inside the plate as the human design carries the arm, 10–11 units — the box
  measure cannot go below that while attached; walk frames 819/820 add the plate swinging into
  the belly, 18.7); pickaxe 42, axe 40, sword 30, dagger 17, spear 8, shortbow 11 (hilts through
  the flipper, 1.0–3.7 — the source's own hand-in-hilt overlap at the same poses is 2.7–4.9);
  net 39 (621: the bag overlaps the body from the short flipper), tinderbox 4, chef's hat 1
  (1.54). `gear/pose-fit-failures.json` lists every failing frame with all measures and, as
  context only, the same item on the human body at the same pose: 356 of the 666 penetration
  failures exceed that design overlap by more than the target (necklace 299, net 37, wooden
  shield 8, bow 1, sword 1); the other 310 are within the overlap the source itself has.
  Provenance of the penetration figure (`embedded_penetration` per failure,
  `over_penetration_box_half_only` in the summary): only 61 of the 666 are decided by the
  carried-box half alone (body geometry behind the item inside its box, no item geometry inside
  the body — square shield 53, wooden shield 6, pickaxe 1, hat 1); the other 605 have item
  geometry genuinely inside the penguin mesh (necklace chain 8–13.6 deep, wooden shield plate
  into the belly in walk frames up to 18.7, square shield up to 11, net bag 10.4, bow 5.3, hilts
  through the flipper 1.5–3.7). An exact mesh-intersection measure would therefore not change
  the verdict for those 605; closing them needs the gear geometry itself adapted (permitted
  minimal deterministic mesh fitting: widening the chain around the neck anchor, keeping the
  plate/bag clear of the body, a hilt seat in the flipper) — not started, no such edit is
  applied yet. The
  pre-declared targets and measures are unchanged; the gate test stays failing (ignored with the
  count) rather than passing on the surface-only placement, and the previous "3 220 item-frames
  pass" figure is superseded: it certified surface clearance, not attachment.
* Bind-pose values now (penetration / gap / attachment, source units): axe 0.96 / 0.71 / 2.00
  (13.6°), dagger 1.20 / 0.23 / 2.00, sword 1.82 / 0.05 / 2.00, pickaxe 1.48 / 0.07 / 2.00,
  spear 0.98 / 0.02 / 2.00, shortbow 0.81 / 0.07 / 2.00, wooden shield 8.40 / 0.14 / 2.00,
  square shield 4.25 / 0.07 / 2.00 (90°), chef's hat 2.93 / 0.51 / 2.00, brass necklace 8.01 /
  1.12 / 2.00 (design overlap on the human body: hat 6.0, shields 10–11, weapons 1–4, necklace
  1.2). `every_m1_worn_model_attaches_at_the_bind_pose` pins this set so a change in either
  direction is noticed.
* Dynamic layers against the independent original references (`tests/dynamic_layers.rs`,
  `assets/reference/osrs240/m1-dynamic`, 12 controlled offline original-client renderings at
  the native **full-HUD zoom 410** — a second original projection beside the frozen
  viewport-only fixtures' 662 at the same 1080 px height; each is applied only to its own
  cases, never one onto the other). Measured with the approved `native_scene_model` profile
  over every pixel outside the three native HUD container rectangles (which the original paints
  over the scene; they are counted and reported, not masked scene geometry): door closed/open
  (object 9398, orientations 0/1), ground piles (single coin, 10 000-coin stack, four items →
  top three by the `lj.es` value/deque rule), fire frames 0 and 3 (sequence 475, 19 cycles),
  roofs outside/inside/hidden, plane-1 view — **11 of 11 rendered cases pixel-identical**
  (1 863 553 / 1 863 553 scene pixels, interior max 0, band 0). The eight phase-independent
  cases match on the published fixture scenes. The three Tutorial plane-0 cases (door
  closed/open, roofs hidden) show three animated flames (object 24969 at 3095,3102; object 196
  torches at 3096,3105 and 3096,3110) whose start phase the original picks with
  `Math.random()` per placement; the independent phase sidecar
  (`assets/reference/osrs240/m1-dynamic/phases`, read-only observations of the ACTIVE
  controller `dy.ac` in byte-identical replays of the three cases) records seq 477 frame 4,
  seq 481 frame 3, seq 481 frame 2, cycle 0 within the frame, drawn at client cycle 0 with
  `lastUpdate` −1. The static fixture scene bakes one frame per flame, so those cases are
  decided on the block-assembled scene (every frame baked with `dy.vn` from the active
  controller, `BlockExport`) at exactly that recorded state:
  `tutorial_flame_cases_match_at_the_recorded_source_phases` (block exports) replays the state
  through `set_scenery_phase(frame, cycle)`, freezes the scenery clock at the recorded one-cycle
  advance (`set_scenery_clock_override`), checks the bake's frame lengths ([7,6,6,6,1] /
  [7,6,6,6,6]) and the post-draw controller state (frame kept, cycle 1 — the terminal 1-cycle
  frame 4 stays put because `rd.az` moves on only when the cycle *exceeds* the length) against
  the sidecar, and matches all three frames pixel-exactly under the full profile with no
  flame-box accounting; the GPU twin does the same. No frame is ever chosen from the reference
  pixels: the earlier frame search is removed (the identical-looking frame it had picked for the
  3096,3105 torch was 0, the recorded one is 3 — identity and timing come only from the source
  record). On the published fixture scene those three cases still report their differences
  confined to the flame placements (0 outside) and defer the decision to the block test. The
  stock normal-camera selector `cz.ch` honours the original hide-roofs preference first
  (`set_hide_roofs`) and reproduces every recorded selector value (3 outside, 0 inside/hidden,
  1 on plane 1).
  `lumbridge-minimap-mapped-chunk` (controlled `rl4.fn` template mapping of source chunk
  402,402 turned 2 quarter turns onto local chunk 6,6) stays **unsupported**: instance chunk
  assembly exists for turn-0 mappings (the M1 Death Office template), but a turned chunk needs
  the terrain re-lit and every model re-lit at the turned orientation from raw inputs the block
  exports do not carry, so it is rejected explicitly rather than approximated. The same eight
  phase-independent cases also pass through the wgpu compute rasterizer on the GB10
  (`dynamic_layer_cases_match_on_the_gpu`: GPU = CPU = source, 0 differing pixels), as do the
  three phased Tutorial cases
  (`tutorial_flame_cases_match_on_the_gpu_at_the_recorded_source_phases`). Chrome 153 (dev
  scenarios `source-*`: the block scene at the fixture base, the sidecar's controller states
  through `setSceneryPhase`/`setSceneryClock`, full-HUD zoom from `fullHudZoomForViewport`, no
  local player body like the source input): **all five Tutorial cases are identical over all
  1 863 553 scene pixels** (the 210 047 native HUD-container pixels are counted separately, not
  masked).
* Minimap (`scene::minimap`, `tests/minimap.rs`): all 15 native minimap captures (5 scenes ×
  planes 0–2) are reproduced pixel-exactly over the scene interior (tiles 5..98) from streamed
  blocks — terrain shapes/colours, bridge tiles, wall/door/diagonal marks, map-scene sprites.
  **All 15 captures are now exact over the whole 512×512 surface — outer five tiles
  included** (interior 0, edge band 0, icon lists 0 mismatches). The block scene runs the
  original terrain pass at assembly time (`scene::terrain`, an exact port of `rl4.ad` +
  `ez.bn` + the shaped-tile constructor `fn`): the squares' raw tiles (`BTER`: underlay /
  overlay id, overlay shape path and rotation, settings, corner height — what `rl4.xl` stores)
  and the shadow footprints of their scenery (`BSHD`, `ci.aq` writes attributed to the casting
  location's origin) are laid into the live loader's 184×184 arrays for the scene's own base —
  only the 104×104 scene filled, the far row/column and margin zero, exactly as the live client
  at the stock draw distance (`rl4.fn` lists no extra squares, `rl4.xl` keeps `0 <= x < 104`) —
  then height-normal light with the (−50, −10, −50) direction and the shadow map, the running
  5-tile HSL box blend of the underlays (`terrain/floors.bin` definitions), `qr`/`hs`/`he`
  colour packing, paints / shaped models (`bn`), and the bridge tile stack turn (`ez.bm`).
  Scenery follows the same loader rule: a location is placed only when its origin tile lies
  strictly inside the scene (`rl4.ws`/`pn`, 1..=102), so edge-tile scenery and out-of-scene
  shadow spill are absent like in the original. Oracle
  (`tests/blocks.rs::terrain_pass_reproduces_the_direct_scene_tiles`): over the five fixture
  bases the rebuilt scene equals the original loader's own per-base export tile for tile —
  50 913 paints, 6 980 shaped tile models (every vertex, colour and texture), every corner
  height and paint/model flag on all four planes. The 3D pinned-scene comparisons, the 11
  dynamic cases and the five Tutorial source cases in Chrome are unchanged (pixel-identical).
  The same pass builds instance scenes from their declared chunks only (blend and light see
  nothing beyond the declared area, undeclared corners hold no height), which is what the
  original instance loader produces; turned chunks (`quarter_turns != 0`) are still rejected —
  the terrain could now turn with the raw data, but the lit scenery models cannot be re-lit at
  a turned orientation from the exported inputs.
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
