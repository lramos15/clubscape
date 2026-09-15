/* tslint:disable */
/* eslint-disable */

export class FrameRecordJs {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly skipped: string;
    completed_at_ms: number;
    cpu_encode_ms: number;
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
    /**
     * Assembles the scene around `base` from loaded blocks; returns the missing squares.
     */
    assemble_scene(base_x: number, base_y: number, now_ms: number): Int32Array;
    /**
     * Original scene base for a player tile (`((tile >> 3) - 6) * 8`).
     */
    static base_for_tile(x: number, y: number): Int32Array;
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
     * Loads a ground-item stack model for quantities `>= min_quantity`.
     */
    load_ground_item(item_id: number, min_quantity: number, bytes: Uint8Array): void;
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
    load_scene(id: string, scene_bytes: Uint8Array, pack_bytes: Uint8Array): void;
    /**
     * Loads an exported sequence (`anim/seq-<id>.bin`, chunks SEQH/SEQL/SEQF/SEQI/SKEL/FRMT).
     * Returns the sequence id.
     */
    load_sequence(bytes: Uint8Array): number;
    /**
     * Whether the tile is within `margin` tiles of the current scene edge (or no scene exists).
     */
    needs_recenter(x: number, y: number, margin: number): boolean;
    /**
     * Creates the renderer on a real WebGPU device. Fails explicitly when unavailable.
     */
    constructor(canvas: HTMLCanvasElement, width: number, height: number, palette_bytes: Uint8Array);
    /**
     * JSON `{"kind":"tile","tile":{...}}` / `{"kind":"entity","id":..,"tile":{...}}` or null.
     */
    pick(x: number, y: number): string | undefined;
    /**
     * JSON report of the current gear fit on the penguin body: per item the bound human label,
     * the penguin label chosen, anchor gap and deepest penetration in source units, and the
     * retarget scale. Empty array before a body/gear is assembled.
     */
    player_fit_report(): string;
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
     * Instanced map flag (`cy.as`): the stock rule then always draws up to the player's plane.
     */
    set_instanced_map(instanced: boolean): void;
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
    timestamps_supported(): boolean;
    unload_block(square: number): void;
    update_world(json: string, now_ms: number): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_framerecordjs_free: (a: number, b: number) => void;
    readonly __wbg_get_framerecordjs_completed_at_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_cpu_encode_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_draw_calls: (a: number) => number;
    readonly __wbg_get_framerecordjs_entities_drawn: (a: number) => number;
    readonly __wbg_get_framerecordjs_gpu_duration_known: (a: number) => number;
    readonly __wbg_get_framerecordjs_gpu_duration_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_primitives: (a: number) => number;
    readonly __wbg_get_framerecordjs_sequence: (a: number) => number;
    readonly __wbg_get_framerecordjs_submitted_at_ms: (a: number) => number;
    readonly __wbg_get_framerecordjs_texture_fallbacks: (a: number) => number;
    readonly __wbg_set_framerecordjs_completed_at_ms: (a: number, b: number) => void;
    readonly __wbg_set_framerecordjs_cpu_encode_ms: (a: number, b: number) => void;
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
    readonly wasmrenderer_assemble_scene: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly wasmrenderer_base_for_tile: (a: number, b: number) => [number, number];
    readonly wasmrenderer_device_epoch: (a: number) => number;
    readonly wasmrenderer_device_lost_reason: (a: number) => [number, number];
    readonly wasmrenderer_frame: (a: number, b: number) => any;
    readonly wasmrenderer_frame_model_fixture: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => any;
    readonly wasmrenderer_frame_player_preview: (a: number, b: number, c: number, d: number) => any;
    readonly wasmrenderer_has_block: (a: number, b: number) => number;
    readonly wasmrenderer_last_frame_triangles: (a: number) => number;
    readonly wasmrenderer_load_block: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly wasmrenderer_load_dynamic_object: (a: number, b: number, c: number, d: number, e: number, f: number, g: any, h: number, i: number) => [number, number];
    readonly wasmrenderer_load_equip_model: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasmrenderer_load_ground_item: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmrenderer_load_model: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmrenderer_load_npc_definition: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
    readonly wasmrenderer_load_npc_pack: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasmrenderer_load_player_body: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number];
    readonly wasmrenderer_load_scene: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly wasmrenderer_load_sequence: (a: number, b: number, c: number) => [number, number, number];
    readonly wasmrenderer_needs_recenter: (a: number, b: number, c: number, d: number) => number;
    readonly wasmrenderer_new: (a: any, b: number, c: number, d: number, e: number) => any;
    readonly wasmrenderer_pick: (a: number, b: number, c: number) => [number, number];
    readonly wasmrenderer_player_fit_report: (a: number) => [number, number];
    readonly wasmrenderer_resize: (a: number, b: number, c: number) => void;
    readonly wasmrenderer_scene_id: (a: number) => [number, number];
    readonly wasmrenderer_scene_placement: (a: number) => [number, number];
    readonly wasmrenderer_set_camera: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number];
    readonly wasmrenderer_set_instanced_map: (a: number, b: number) => void;
    readonly wasmrenderer_set_roof_context: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmrenderer_set_roof_mode: (a: number, b: number) => void;
    readonly wasmrenderer_set_top_plane_override: (a: number, b: number) => void;
    readonly wasmrenderer_squares_for_base: (a: number, b: number) => [number, number];
    readonly wasmrenderer_timestamps_supported: (a: number) => number;
    readonly wasmrenderer_unload_block: (a: number, b: number) => void;
    readonly wasmrenderer_update_world: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___js_sys_3b7301898fbf4e22___Function_fn_wasm_bindgen_765df639e0572edc___JsValue_____wasm_bindgen_765df639e0572edc___sys__Undefined___js_sys_3b7301898fbf4e22___Function_fn_wasm_bindgen_765df639e0572edc___JsValue_____wasm_bindgen_765df639e0572edc___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___JsValue__core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___sys__JsNullable_wgpu_fb237351f69b1e72___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___sys__JsNullable_wgpu_fb237351f69b1e72___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true__63: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___sys__JsNullable_wgpu_fb237351f69b1e72___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true__64: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wgpu_fb237351f69b1e72___backend__webgpu__webgpu_sys__gen_GpuDeviceLostInfo__GpuDeviceLostInfo______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wgpu_fb237351f69b1e72___backend__webgpu__webgpu_sys__gen_GpuDeviceLostInfo__GpuDeviceLostInfo______true__62: (a: number, b: number, c: any) => void;
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
