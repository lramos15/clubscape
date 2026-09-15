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
| `scene::draw` (roofs, live layers) | `ez.or/ei` + `jz`, `cz.ch`, `eq/fk` item layers, `ez.ck` wall replace | Original roof removal (mode bits 1/2/4/8 incl. the camera-line Bresenham), the stock top-plane rule, ground-item layers at the three original draw points, door wall overrides. |
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
# lints as CI runs them
cargo clippy -p clubscape-renderer --release --features gpu --all-targets -- -D warnings
cargo clippy -p clubscape-renderer --features web --target wasm32-unknown-unknown -- -D warnings
# browser artefact (also generates web/renderer/pkg and web/renderer/dist)
web/renderer/build.sh
```

Runtime inputs live in `assets/compiled/render` (see `tools/render-assets/README.md`). Tests read
published buffers through `tests/common/mod.rs`: raw scene buffers are published only as
deterministic gzip twins and are inflated on the fly, so a clean checkout runs every scene and
NPC case; a declared input that is missing fails the test. No test skips a mandatory case at
run time — suites over unpublished, reproducible exports (world blocks, pinned validation
twins, per-frame bakes) are `#[ignore]`d with the reproduction command and fail on missing
files when run with `--include-ignored`.

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
  gear fit, live layers (items, fire, doors, roof modes), the stock top-plane rule, the interface
  preview projection.
* `tests/gpu.rs` (`--features gpu`) – GPU output equals the CPU port and the source PNGs for the
  five scenes and all 58 model captures (4 tree yaws + 54 pack frames); coverage alpha and the
  asynchronous readback. Logged GPU times are the completed frame's timestamp-query span or
  are labelled unavailable; wall time is reported separately.
* `tests/blocks.rs` (ignored by default) – scenes assembled from world blocks are pixel-identical
  to the original loader's direct export at all five fixture bases (animated scenery pinned to
  frame 0 on both sides); recentering around the player keeps the whole visible world.

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
| `assemble_scene(baseX, baseY, nowMs)` → missing squares | Assembles the scene from loaded blocks (unexported squares stay empty like unloaded map squares in the original) and restarts the scenery animation clock. |
| `resize(width, height)` | Reconfigures the surface and projection centre. |
| `set_camera(x, height, y, pitch, yaw, zoom, far)` | World units / 16384 per turn; rejects zoom ≤ 0 or far < 50. |
| `update_world(json, nowMs)` | Shared `WorldView` JSON (plus the optional `dynamicObjects` extension). Player and `npc` entities become scene actors with the original tile-span registration and facing from tile steps. NPCs with a loaded definition play their source stand/walk (from tile steps) or death (`hitpoints == 0`) sequence through the skeletal port; NPCs with only a baked pack use the numeric `animation` string. The player is the approved penguin body wearing `player.equipment[].item.sourceId` models, animated by the activity (`walking` → 819/824, `gathering` → 879/625/621 by adjacent tree/rock/fishing spot, `producing` → 733/897/896/899/898 by adjacent fire/range/furnace/anvil, `fighting` → 386/390/422/426 by weapon, `casting` → 711, `hitpoints == 0` → 836, otherwise 808). `temporary_object` entities (fires) animate from first appearance; `groundItems` draw the tile's top three stacks; `dynamicObjects[].doorOpen/quarterTurns` swap door walls. Entities in another `instance` than the player are skipped. |
| `load_sequence(bytes)` → id | Exported skeletal sequence (`anim/seq-<id>.bin`). |
| `load_npc_definition(recordJson, baseBytes)` → npc id | `manifest.npc_definitions[i]` as JSON with its lit base model. |
| `load_player_body(penguinBase, widthScale, heightScale, nativeSequences, humanReference)` | Installs the approved player body (NPC 2063 base at 75/128, native sequences 5668/5666) and the human reference body used to retarget player-appearance sequences. |
| `load_equip_model(itemId, bytes)` | Worn item model (`models/item-<id>-equip.bin`). |
| `load_dynamic_object(objectId, type, orientation, plain, frames, frameLengths)` | Door/fire/state object variant with optional baked frames. |
| `load_ground_item(itemId, minQuantity, bytes)` | Ground stack model for quantities ≥ `minQuantity`. |
| `set_top_plane_override(limit \| undefined)`, `set_instanced_map(bool)` | `br`: the approved fixtures pinned the `dh` plane argument to 0; `undefined` applies the stock `cz.ch` rule (all planes unless a roof-flagged tile of the player's plane lies on the camera→player line at pitch < 2480, then the player's plane; always the player's plane in instanced maps). |
| `set_roof_mode(bits)`, `set_roof_context(hx, hy, dx, dy)` | Original roof removal (`ez.ny`): 1 player tile, 2 hovered tile, 4 walk destination, 8 camera line. 0 = stock. |
| `player_fit_report()` → JSON | Per worn item: bound human label, penguin label, anchor gap and deepest body penetration in source units, retarget scale. |
| `scene_placement()` → JSON | `{baseX, baseY, sizeTiles: 104, blocks}` for HUD/minimap helpers. |
| `frame_player_preview(optionsJson, nowMs)` → `Promise<Uint8ClampedArray \| undefined>` | Model-only interface preview of the current body + gear through the original type-6 component draw (interface 679:73 defaults from `manifest.model_widgets`: 480×315 parent layer, component centre, zoom 512, model zoom 450, offsetY2 175, content type 328 pitch 150 + 20 ms-cycle sway). Tightly packed RGBA with alpha 255 only where geometry was written; resolves after GPU completion and readback mapping. |
| `frame(nowMs)` → `Promise<FrameRecordJs>` | Builds, submits and presents one frame; resolves after GPU completion with `sequence`, `submitted_at_ms`, `completed_at_ms`, `draw_calls`, `primitives`, `cpu_encode_ms`, `gpu_duration_ms`/`gpu_duration_known`, `entities_drawn`, `texture_fallbacks`, `skipped`. Rejects on device loss, uncaptured WebGPU errors or pipeline validation failures. |
| `frame_model_fixture(model, npc, sequence, frame, yaw, cameraY, cameraZ)` | Developer replay of an approved model capture (legacy draw, zoom 1024, background 0x303030). |
| `pick(x, y)` → JSON or `undefined` | Exact fill coverage of the last frame. Actors: `{"kind":"entity","id":<WorldView id>,"tile"}`. Scenery: `{"kind":"tile","tile":<object origin tile>,"scenery":{"objectId","type","spanX","spanY"}}`, or `kind: "entity"` with the WorldView `object` entity id when the shell listed one on that footprint. Ground: `{"kind":"tile","tile"}`. |
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
* `manifest.model_widgets`: original if3 model components decoded by `lw.ag` (interface 679
  component 73: 136×192 at originalY 32 in the 480×315 layer, model zoom 450, offsetY2 175,
  content type 328).

## Known deviations and open items

* Zone dynamic lists (`ZDYN`) are exported but empty in every fixture; the `si`/`dy` semantics
  are ported as read but untested against source pixels.
* Actor heights sample the original tile height field (bilinear, bridge-aware); the fixtures
  contain no live actors to compare against. Live-layer behaviour (gear, action sequences,
  ground items, fires, doors, roof modes, stock top plane) is proven by geometry/pixel-change
  tests and browser captures, not by approved source images — the approved pack has none.
* The skeletal port reproduces all 54 baked goblin/penguin frames vertex- and pixel-exactly;
  Maya-skeletal sequences (`ou.bp >= 0`) are not supported (none are required for M1).
* Player action motions are the original human sequences retargeted onto the penguin labels
  (translation scale 161/196, bone table in `actor::HUMAN_TO_PENGUIN_LABELS`); this is a
  technical retarget of source timing/geometry, not a source capture of a penguin performing them.
* Gear fit on the penguin body (source units, unscaled): bronze axe gap 1.46 / penetration 1.01,
  bronze sword 0.65 / 1.00, brass necklace 0 / 0 meet the ≤2 / ≤1 targets; chef's hat (2.0 /
  6.56) and wooden shield (2.0 / 5.73) exceed the penetration target because the hat sits on the
  penguin's much wider head and the shield on the flipper — reported unmet, not hidden.
* Run animation (824) needs a running flag the shared `WorldView` does not carry; `walking`
  activity plays 819. Door states need the protocol `dynamic_objects` forwarded by the shell as
  the optional `dynamicObjects` extension (see `web/renderer/README.md`).
* Animated scenery uses native frame bakes (`dy.vn` per source frame) with the original sequence
  advance ported (`rd.az`: frame lengths, loop step, one-shot reset to the plain model). Start
  phases are per-placement pseudo-random like the original `Math.random` seeds, so they are not
  pixel-comparable to a capture; the block tests pin frame 0 on both sides.
* World block size: the 61 M1 squares total ≈ 75 MB gzip (the sea square 12078 alone ≈ 10 MB
  because its wave scenery has 124 baked frames of 3.2k-vertex models). Blocks stay out of git
  (`export.py --profile blocks` reproduces them; hashes are in the manifest).
