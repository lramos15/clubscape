/* tslint:disable */
/* eslint-disable */

export class FrameRecordJs {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly skipped: string;
    completed_at_ms: number;
    /**
     * CPU breakdown of `cpu_encode_ms`: scene build (traversal + projection), GPU-layout
     * packing, buffer upload + compute encode/submit, canvas acquire/blit/present.
     */
    cpu_build_ms: number;
    cpu_encode_ms: number;
    cpu_pack_ms: number;
    cpu_present_ms: number;
    cpu_upload_ms: number;
    draw_calls: number;
    entities_drawn: number;
    gpu_duration_known: boolean;
    gpu_duration_ms: number;
    primitives: number;
    sequence: number;
    submitted_at_ms: number;
    texture_fallbacks: number;
}

export class WasmRenderer {
    free(): void;
    [Symbol.dispose](): void;
    adapter_info(): string;
    add_texture(bytes: Uint8Array): number;
    apply_native_camera(json: string): string;
    /**
     * Assembles the scene around `base` from loaded blocks; returns the missing squares.
     */
    assemble_scene(base_x: number, base_y: number, now_ms: number): Int32Array;
    /**
     * Original scene base for a player tile (`((tile >> 3) - 6) * 8`).
     */
    static base_for_tile(x: number, y: number): Int32Array;
    camera_scene(): string;
    camera_source(): string;
    device_epoch(): number;
    /**
     * Non-null once the WebGPU device reported loss; frames then fail explicitly.
     */
    device_lost_reason(): string | undefined;
    /**
     * Builds and submits one frame; resolves when the GPU queue reports the work complete.
     */
    frame(now_ms: number): Promise<any>;
    /**
     * Developer fixture replay of an approved model capture (legacy draw, zoom 1024,
     * background 0x303030). `npc`/`sequence`/`frame` select a baked NPC frame, otherwise
     * `model` names a model loaded with `load_model`.
     */
    frame_model_fixture(model: string, npc: number, sequence: number, frame: number, yaw: number, camera_y: number, camera_z: number): Promise<any>;
    /**
     * Renders the model-only player preview an interface model component shows and resolves
     * with tightly packed RGBA8 pixels (`Uint8ClampedArray`, `width * height * 4`; alpha 255
     * only where the model covered the pixel) after the GPU completed the work and the
     * readback mapped. `options_json` fields (all optional, defaults = the exported interface
     * 679 component 73 draw): `width`, `height`, `centerX`, `centerY`, `contentType` (328 =
     * character-design sway/pitch overrides), `rasterizerZoom`, `modelZoom`, `rotationX`,
     * `rotationY`, `rotationZ`, `offsetX`, `offsetY`, `sequence`, `frame`. Resolves
     * `undefined` when no player body is loaded.
     */
    frame_player_preview(options_json: string, now_ms: number): Promise<any>;
    has_block(square: number): boolean;
    has_map_scenes(): boolean;
    has_minimap_block(square: number): boolean;
    /**
     * Whether the last `update_world` changed the instance layout (`WorldView.instanceLayout`),
     * so the block scene must be reassembled before it matches the world.
     */
    instance_layout_changed(): boolean;
    last_frame_triangles(): number;
    /**
     * Loads a world block (64x64 map square) for scene assembly.
     */
    load_block(square: number, block_bytes: Uint8Array, pack_bytes: Uint8Array): void;
    /**
     * Loads one dynamic object variant (`manifest.dynamic_objects[i].variants[j]`): the plain
     * model plus optional baked animation frames (`Array<Uint8Array>`) with their lengths.
     */
    load_dynamic_object(object_id: number, kind: number, orientation: number, plain: Uint8Array, frames: Array<any>, frame_lengths: Int32Array): void;
    /**
     * Loads an equippable item's worn model (`models/item-<id>-equip.bin`).
     */
    load_equip_model(item_id: number, bytes: Uint8Array): void;
    /**
     * Loads the floor underlay/overlay definitions (`terrain/floors.bin`, manifest
     * `floor_definitions`). Block scenes assembled afterwards run the original terrain pass
     * (`rl4.ad`: blend, light, tile shapes) from the squares' raw terrain at the scene's own
     * base, so every tile — the outer five included — is what the live client builds. Returns
     * `[underlays, overlays]`.
     */
    load_floor_defs(bytes: Uint8Array): Uint32Array;
    /**
     * Loads a ground-item stack model for quantities `>= min_quantity`.
     */
    load_ground_item(item_id: number, min_quantity: number, bytes: Uint8Array): void;
    /**
     * Loads the original map-element minimap sprites (`minimap/mapicons.bin`, `ps.as(false)`
     * per element the original shows on the minimap). Returns the sprite count.
     */
    load_map_icons(bytes: Uint8Array): number;
    /**
     * Loads the original map-scene sprites and tile-shape masks (`minimap/mapscenes.bin`) the
     * minimap needs.
     */
    load_map_scenes(bytes: Uint8Array): void;
    /**
     * Loads a square's minimap sidecar (`minimap/blocks/<square>.bin`: wall placement configs
     * and object-definition map fields). Order relative to `load_block` does not matter.
     */
    load_minimap_block(square: number, bytes: Uint8Array): void;
    load_model(id: string, bytes: Uint8Array): void;
    /**
     * Loads an NPC definition (`manifest.npc_definitions[i]` as JSON) with its lit base model.
     * Returns the NPC id.
     */
    load_npc_definition(record_json: string, base_bytes: Uint8Array): number;
    load_npc_pack(npc_id: number, bytes: Uint8Array): void;
    /**
     * Installs the approved player body: the penguin base model (NPC 2063, model 21547) at its
     * definition scales, the sequences that drive it natively (stand/walk), and the human
     * reference body used to retarget player-appearance sequences onto penguin labels.
     */
    load_player_body(penguin_base: Uint8Array, width_scale: number, height_scale: number, native_sequences: Int32Array, human_reference: Uint8Array): void;
    /**
     * Loads the precomputed per-pose gear fit table (`gear/pose-fits.json`, manifest
     * `gear_pose_fits`) for the approved body NPC. Rejected (error) when it is for another body,
     * another target pair or stale against the loaded sequences; without it every player frame
     * solves its fit live on first display. Returns the accepted item × sequence entry count.
     */
    load_pose_fit_table(json: string, body_npc: number): number;
    load_scene(id: string, scene_bytes: Uint8Array, pack_bytes: Uint8Array): void;
    /**
     * Loads an exported sequence (`anim/seq-<id>.bin`, chunks SEQH/SEQL/SEQF/SEQI/SKEL/FRMT).
     * Returns the sequence id.
     */
    load_sequence(bytes: Uint8Array): number;
    /**
     * RGBA8 pixels of every map-element sprite concatenated in `map_icon_sprites()` order
     * (width × height × 4 each). The original sprite blit (`ym.af`) skips pixels whose value is
     * 0, so those get alpha 0 and every other pixel alpha 255.
     */
    map_icon_pixels(): Uint8ClampedArray;
    /**
     * The loaded map-element sprites (JSON array of `{element, width, height, offsetX, offsetY,
     * maxWidth, maxHeight, category}` in the order `map_icon_pixels()` packs them). The HUD
     * draws them over the minimap surface with the original rule (`minimap_icon_placement` /
     * `minimap_icon_placements` compute it exactly): `dx = ((x << 7) + 64 - playerFineX) *
     * scale` in minimap pixels (4 px per tile at the stock 1/32), rotated by the map angle,
     * canvas top-left at `(W/2 + dx' - maxWidth/2, H/2 - dy' - maxHeight/2)`; skipped beyond
     * 80 px (20 tiles), mask-clipped beyond 50 px.
     */
    map_icon_sprites(): string;
    /**
     * Exact `client.zr` → `bo.as` placement of one minimap marker (JSON `{x, y, drawX, drawY,
     * clipped, dx, dy}` in **minimap pixels** relative to the widget origin, or `undefined`
     * when the marker is out of range). `tile_x/tile_y` and `player_fine_x/player_fine_y`
     * share one tile domain (world or scene-local): the player's fine position is
     * `tile * 128 + 64` (the renderer draws actors at tile centres); `scale` is the minimap
     * zoom (stock `0.03125` = 1/32: 4 px per tile; RuneLite zoom `z` → `z / 128`);
     * `minimap_angle` the camera yaw in 16384 units (`client.jv`); `widget_w/h` the minimap
     * widget sprite size; the sprite fields come from `map_icon_sprites()` (`maxWidth`,
     * `maxHeight`, `offsetX`, `offsetY`). Cut-off `dx² + dy² > 6400` (80 px = 20 tiles at the
     * stock scale), mask-clipped beyond 2500 (50 px).
     */
    minimap_icon_placement(tile_x: number, tile_y: number, player_fine_x: number, player_fine_y: number, scale: number, minimap_angle: number, widget_w: number, widget_h: number, sprite_max_w: number, sprite_max_h: number, sprite_offset_x: number, sprite_offset_y: number): string | undefined;
    /**
     * Every marker of the current `minimap_surface()` placed for the widget (JSON array of
     * `{element, tileX, tileY, x, y, drawX, drawY, clipped, dx, dy}` — `x/y` per
     * `minimap_icon_placement`), using the loaded sprites' canvas sizes/offsets and the
     * current camera yaw as the minimap angle; markers out of range or without a loaded sprite
     * are omitted (the latter counted in `missingSprites`). Player at the centre of
     * `(player_tile_x, player_tile_y)` (world tiles, like `minimap_surface().icons`).
     */
    minimap_icon_placements(player_tile_x: number, player_tile_y: number, scale: number, widget_w: number, widget_h: number): string;
    /**
     * Coverage mask (`Uint8Array`, width * height): 1 where the original sweep drew map data,
     * 0 where the source fill value survived (no tile).
     */
    minimap_mask(): Uint8Array;
    /**
     * RGBA8 pixels (`Uint8ClampedArray`, width * height * 4, alpha 255) of the surface
     * `minimap_surface()` last described.
     */
    minimap_pixels(): Uint8ClampedArray;
    /**
     * Metadata of the source minimap surface for the current scene and plane (JSON:
     * `width`, `height`, `scale`, `marginX`, `marginY`, `baseX`, `baseY`, `plane`, `revision`,
     * `complete`, `stats {terrainTiles, wallMarks, diagonalMarks, mapScenes, unresolved}`,
     * `notes[]`, `icons[{x, y, plane, element}]`). Draws (or reuses the cached raster) with the
     * original `client.bm` port; rejects when no scene, no map-scene asset or no sidecars.
     * Pixels and mask follow from `minimap_pixels()` / `minimap_mask()` for the same revision.
     */
    minimap_surface(): string;
    /**
     * Whether the tile is within `margin` tiles of the current scene edge (or no scene exists).
     */
    needs_recenter(x: number, y: number, margin: number): boolean;
    /**
     * Creates the renderer on a real WebGPU device. Fails explicitly when unavailable.
     */
    constructor(canvas: HTMLCanvasElement, width: number, height: number, palette_bytes: Uint8Array);
    /**
     * Whether the last world view carried `game.observer.v1` fields (`running` / `action`).
     * False means an older observer: movement is inferred from ticks and every action motion
     * must come from `animation` or forwarded events.
     */
    observer_v1(): boolean;
    /**
     * JSON `{"kind":"tile","tile":{...}}` / `{"kind":"entity","id":..,"tile":{...}}` or null.
     */
    pick(x: number, y: number): string | undefined;
    /**
     * JSON report of the current gear fit on the penguin body: per item the bound human label,
     * the penguin label chosen, `penetration` (deepest body vertex inside the item's box, target
     * ≤ 1), `gap` (item↔body clearance, target ≤ 2), `anchorShift` (contact-solve translation
     * from the retargeted design position), `designPenetration` (the same box measure on the
     * human body the item was designed for) in source units, and the retarget scale. Empty
     * array before a body/gear is assembled.
     */
    player_fit_report(): string;
    /**
     * Per-pose gear fits of the player frames drawn since the gear last changed (JSON array of
     * `{sequence, frame, itemId, slot, shift, direction, rotation, precomputed, penetration,
     * gap, attachmentGap, anchorClearance, meetsTargets}`): `penetration`
     * is the carried-bind-box depth (≤ 1), `gap` the item↔body surface clearance (≤ 2) and
     * `attachmentGap` the grip's distance from where the posed anchor bone carries it (≤ 2);
     * entries with `meetsTargets: false` are the exact current fit failures.
     */
    player_pose_fits(): string;
    /**
     * Whether the player is running: the observer contract's executed `running` when the world
     * view carries it, else the original two-tiles-per-server-tick rule (or the `run` setting
     * when ticks are unavailable).
     */
    player_running(): boolean;
    /**
     * Item definition fields the original pile selection (`lj.es`) reads: shop value `op.ef`
     * and the stackable flag (`manifest.ground_items[].price` / `.stackable`). The pile's top
     * is the greatest value (times quantity + 1 when stackable), then two other distinct ids.
     */
    register_ground_item_definition(item_id: number, price: number, stackable: boolean): void;
    resize(width: number, height: number): void;
    scene_id(): string | undefined;
    /**
     * Current scene placement for HUD helpers (minimap): JSON
     * `{"baseX","baseY","sizeTiles":104,"blocks":bool}` or `undefined` without a scene.
     */
    scene_placement(): string | undefined;
    /**
     * Camera in world units (tile * 128), 16384 units per turn, height negative-up.
     */
    set_camera(x: number, height: number, y: number, pitch: number, yaw: number, zoom: number, far: number): void;
    /**
     * Kit ids present in the source cache (manifest `sequence_hand_overrides.values[*]` with
     * `kind: "kit"` and `kit_exists: true`) for the `lc.bd` hand-override decode: a sequence
     * whose hand item names a kit the cache lacks draws nothing in that slot.
     */
    set_hand_override_kits(kits: Int32Array): void;
    /**
     * Developer fixture control only: draw no body for the local player, as in the controlled
     * original dynamic-layer references (`assets/reference/osrs240/m1-dynamic`). Off by default.
     */
    set_hide_local_player_body(hidden: boolean): void;
    /**
     * The original "hide roofs" preference (`cy.as`): the stock top-plane rule then draws up
     * to the player's plane only. Off by default.
     */
    set_hide_roofs(hidden: boolean): void;
    /**
     * Instanced map flag (`cy.as`): the stock rule then always draws up to the player's plane.
     */
    set_instanced_map(instanced: boolean): void;
    /**
     * Developer-only: derive action motions from the activity string and adjacent scenery when
     * the world view supplies no source animation. Off by default; not final M1 logic.
     */
    set_motion_fallback(enabled: boolean): void;
    /**
     * Sets the plane frames and the minimap are drawn for when no WorldView supplies a player
     * (developer fixtures); the WorldView's player plane overrides it on the next update.
     */
    set_plane(plane: number): void;
    /**
     * Hovered world tile and walk destination consulted by roof modes 2 and 4 (`undefined`
     * clears either).
     */
    set_roof_context(hovered_x?: number | null, hovered_y?: number | null, destination_x?: number | null, destination_y?: number | null): void;
    /**
     * Original roof-removal mode bits (1 player tile, 2 hovered tile, 4 walk destination,
     * 8 camera line); 0 draws every roof like the stock client the fixtures were captured with.
     */
    set_roof_mode(mode: number): void;
    /**
     * Developer fixture control only: freezes the scenery animation clock at `cycles` client
     * cycles since scene start (negative = real time again), so frames render at exactly the
     * cycle an original capture was drawn at. Actors/fires/frame timestamps are untouched.
     */
    set_scenery_clock_override(cycles: number): void;
    /**
     * Developer fixture control only: replays an original capture's recorded animated-scenery
     * controller state (`dy.ac`: frame and cycle within the frame at scene start) for every
     * animated instance of `object_id` on the world tile. Returns how many instances were set.
     */
    set_scenery_phase(plane: number, x: number, y: number, object_id: number, frame: number, cycle: number): number;
    /**
     * Top drawn plane override (`br`, the original `dh` plane argument). The approved fixture
     * captures pinned it to 0 (ground plane only); `undefined` restores the stock live rule
     * (`cz.ch`: all planes unless a roof-flagged tile of the player's plane lies on the
     * camera→player line at pitch < 2480, then the player's plane).
     */
    set_top_plane_override(limit?: number | null): void;
    /**
     * Map squares (`x << 8 | y`) a scene at `base` needs, as the original loader requests them.
     */
    static squares_for_base(base_x: number, base_y: number): Int32Array;
    /**
     * Map squares a scene at `base` needs under the current instance layout: only the squares
     * holding declared source chunks inside an instance, `squares_for_base` otherwise.
     */
    squares_needed(base_x: number, base_y: number): Int32Array;
    /**
     * Statistics of the terrain pass of the current block scene (JSON `{paints, tileModels,
     * missingOverlays, missingUnderlays}`), or `undefined` when no block scene is loaded.
     * Missing raw terrain or floor definitions are assembly errors, not approximate scenes.
     */
    terrain_rebuilt(): string | undefined;
    timestamps_supported(): boolean;
    /**
     * Source actions the last `update_world` reported without a bound animation
     * (`ActorActionView.animation: null`), JSON array of `{actorId, id, activity, actionId,
     * recipeId, styleId, spellId}`: the exact identities awaiting a backend binding. Empty
     * when every reported action names its source sequence.
     */
    unbound_actions(): string;
    /**
     * Actors whose reported state implies an action but whose source motion was not supplied
     * in the last world view (JSON array of strings). Empty when every motion is explicit.
     */
    unknown_motions(): string;
    unload_block(square: number): void;
    update_world(json: string, now_ms: number): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_framerecordjs_free: (a: number, b: number) => void;
    readonly __wbg_get_framerecordjs_completed_at_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_cpu_build_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_cpu_encode_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_cpu_pack_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_cpu_present_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_cpu_upload_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_draw_calls: (a: number) => number;
    readonly __wbg_get_framerecordjs_entities_drawn: (a: number) => number;
    readonly __wbg_get_framerecordjs_gpu_duration_known: (a: number) => number;
    readonly __wbg_get_framerecordjs_gpu_duration_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_primitives: (a: number) => number;
    readonly __wbg_get_framerecordjs_sequence: (a: number) => number;
    readonly __wbg_get_framerecordjs_submitted_at_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_texture_fallbacks: (a: number) => number;
    readonly __wbg_set_framerecordjs_completed_at_ms: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_cpu_build_ms: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_cpu_encode_ms: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_cpu_pack_ms: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_cpu_present_ms: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_cpu_upload_ms: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_draw_calls: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_entities_drawn: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_gpu_duration_known: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_gpu_duration_ms: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_primitives: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_sequence: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_submitted_at_ms: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_texture_fallbacks: (a: number, b: number) => void;
    readonly __wbg_wasmrenderer_free: (a: number, b: number) => void;
    readonly framerecordjs_skipped: (a: number) => [number, number];
    readonly wasmrenderer_adapter_info: (a: number) => [number, number];
    readonly wasmrenderer_add_texture: (a: number, b: number, c: number) => [number, number, number];
    readonly wasmrenderer_apply_native_camera: (a: number, b: number, c: number) => [number, number, number, number];
    readonly wasmrenderer_assemble_scene: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly wasmrenderer_base_for_tile: (a: number, b: number) => [number, number];
    readonly wasmrenderer_camera_scene: (a: number) => [number, number, number, number];
    readonly wasmrenderer_camera_source: (a: number) => [number, number, number, number];
    readonly wasmrenderer_device_epoch: (a: number) => number;
    readonly wasmrenderer_device_lost_reason: (a: number) => [number, number];
    readonly wasmrenderer_frame: (a: number, b: number) => any;
    readonly wasmrenderer_frame_model_fixture: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => any;
    readonly wasmrenderer_frame_player_preview: (a: number, b: number, c: number, d: number) => any;
    readonly wasmrenderer_has_block: (a: number, b: number) => number;
    readonly wasmrenderer_has_map_scenes: (a: number) => number;
    readonly wasmrenderer_has_minimap_block: (a: number, b: number) => number;
    readonly wasmrenderer_instance_layout_changed: (a: number) => number;
    readonly wasmrenderer_last_frame_triangles: (a: number) => number;
    readonly wasmrenderer_load_block: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly wasmrenderer_load_dynamic_object: (a: number, b: number, c: number, d: number, e: number, f: number, g: any, h: number, i: number) => [number, number];
    readonly wasmrenderer_load_equip_model: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasmrenderer_load_floor_defs: (a: number, b: number, c: number) => [number, number, number, number];
    readonly wasmrenderer_load_ground_item: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmrenderer_load_map_icons: (a: number, b: number, c: number) => [number, number, number];
    readonly wasmrenderer_load_map_scenes: (a: number, b: number, c: number) => [number, number];
    readonly wasmrenderer_load_minimap_block: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasmrenderer_load_model: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmrenderer_load_npc_definition: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
    readonly wasmrenderer_load_npc_pack: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasmrenderer_load_player_body: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number];
    readonly wasmrenderer_load_pose_fit_table: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wasmrenderer_load_scene: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly wasmrenderer_load_sequence: (a: number, b: number, c: number) => [number, number, number];
    readonly wasmrenderer_map_icon_pixels: (a: number) => any;
    readonly wasmrenderer_map_icon_sprites: (a: number) => [number, number];
    readonly wasmrenderer_minimap_icon_placement: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number) => [number, number];
    readonly wasmrenderer_minimap_icon_placements: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly wasmrenderer_minimap_mask: (a: number) => [number, number, number];
    readonly wasmrenderer_minimap_pixels: (a: number) => [number, number, number];
    readonly wasmrenderer_minimap_surface: (a: number) => [number, number, number, number];
    readonly wasmrenderer_needs_recenter: (a: number, b: number, c: number, d: number) => number;
    readonly wasmrenderer_new: (a: any, b: number, c: number, d: number, e: number) => any;
    readonly wasmrenderer_observer_v1: (a: number) => number;
    readonly wasmrenderer_pick: (a: number, b: number, c: number) => [number, number];
    readonly wasmrenderer_player_fit_report: (a: number) => [number, number];
    readonly wasmrenderer_player_pose_fits: (a: number) => [number, number];
    readonly wasmrenderer_player_running: (a: number) => number;
    readonly wasmrenderer_register_ground_item_definition: (a: number, b: number, c: number, d: number) => void;
    readonly wasmrenderer_resize: (a: number, b: number, c: number) => void;
    readonly wasmrenderer_scene_id: (a: number) => [number, number];
    readonly wasmrenderer_scene_placement: (a: number) => [number, number];
    readonly wasmrenderer_set_camera: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number];
    readonly wasmrenderer_set_hand_override_kits: (a: number, b: number, c: number) => void;
    readonly wasmrenderer_set_hide_local_player_body: (a: number, b: number) => void;
    readonly wasmrenderer_set_hide_roofs: (a: number, b: number) => void;
    readonly wasmrenderer_set_instanced_map: (a: number, b: number) => void;
    readonly wasmrenderer_set_motion_fallback: (a: number, b: number) => void;
    readonly wasmrenderer_set_plane: (a: number, b: number) => void;
    readonly wasmrenderer_set_roof_context: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmrenderer_set_roof_mode: (a: number, b: number) => void;
    readonly wasmrenderer_set_scenery_clock_override: (a: number, b: number) => void;
    readonly wasmrenderer_set_scenery_phase: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly wasmrenderer_set_top_plane_override: (a: number, b: number) => void;
    readonly wasmrenderer_squares_for_base: (a: number, b: number) => [number, number];
    readonly wasmrenderer_squares_needed: (a: number, b: number, c: number) => [number, number];
    readonly wasmrenderer_terrain_rebuilt: (a: number) => [number, number];
    readonly wasmrenderer_timestamps_supported: (a: number) => number;
    readonly wasmrenderer_unbound_actions: (a: number) => [number, number];
    readonly wasmrenderer_unknown_motions: (a: number) => [number, number];
    readonly wasmrenderer_unload_block: (a: number, b: number) => void;
    readonly wasmrenderer_update_world: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___js_sys_3b7301898fbf4e22___Function_fn_wasm_bindgen_765df639e0572edc___JsValue_____wasm_bindgen_765df639e0572edc___sys__Undefined___js_sys_3b7301898fbf4e22___Function_fn_wasm_bindgen_765df639e0572edc___JsValue_____wasm_bindgen_765df639e0572edc___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___JsValue__core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___sys__JsNullable_wgpu_e80dab64ff4a2d4d___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___sys__JsNullable_wgpu_e80dab64ff4a2d4d___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true__104: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___sys__JsNullable_wgpu_e80dab64ff4a2d4d___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true__105: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wgpu_e80dab64ff4a2d4d___backend__webgpu__webgpu_sys__gen_GpuDeviceLostInfo__GpuDeviceLostInfo______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wgpu_e80dab64ff4a2d4d___backend__webgpu__webgpu_sys__gen_GpuDeviceLostInfo__GpuDeviceLostInfo______true__103: (a: number, b: number, c: any) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
