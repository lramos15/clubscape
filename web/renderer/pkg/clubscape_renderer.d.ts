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
    device_epoch(): number;
    /**
     * Non-null once the WebGPU device reported loss; frames then fail explicitly.
     */
    device_lost_reason(): string | undefined;
    /**
     * Builds and submits one frame; resolves when the GPU queue reports the work complete.
     */
    frame(now_ms: number): Promise<any>;
    last_frame_triangles(): number;
    load_npc_pack(npc_id: number, bytes: Uint8Array): void;
    load_scene(id: string, scene_bytes: Uint8Array, pack_bytes: Uint8Array): void;
    /**
     * Creates the renderer on a real WebGPU device. Fails explicitly when unavailable.
     */
    constructor(canvas: HTMLCanvasElement, width: number, height: number, palette_bytes: Uint8Array);
    /**
     * JSON `{"kind":"tile","tile":{...}}` / `{"kind":"entity","id":..,"tile":{...}}` or null.
     */
    pick(x: number, y: number): string | undefined;
    resize(width: number, height: number): void;
    scene_id(): string | undefined;
    /**
     * Camera in world units (tile * 128), 16384 units per turn, height negative-up.
     */
    set_camera(x: number, height: number, y: number, pitch: number, yaw: number, zoom: number, far: number): void;
    timestamps_supported(): boolean;
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
    readonly wasmrenderer_device_epoch: (a: number) => number;
    readonly wasmrenderer_device_lost_reason: (a: number) => [number, number];
    readonly wasmrenderer_frame: (a: number, b: number) => any;
    readonly wasmrenderer_last_frame_triangles: (a: number) => number;
    readonly wasmrenderer_load_npc_pack: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasmrenderer_load_scene: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly wasmrenderer_new: (a: any, b: number, c: number, d: number, e: number) => any;
    readonly wasmrenderer_pick: (a: number, b: number, c: number) => [number, number];
    readonly wasmrenderer_resize: (a: number, b: number, c: number) => void;
    readonly wasmrenderer_scene_id: (a: number) => [number, number];
    readonly wasmrenderer_set_camera: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number];
    readonly wasmrenderer_timestamps_supported: (a: number) => number;
    readonly wasmrenderer_update_world: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___js_sys_3b7301898fbf4e22___Function_fn_wasm_bindgen_765df639e0572edc___JsValue_____wasm_bindgen_765df639e0572edc___sys__Undefined___js_sys_3b7301898fbf4e22___Function_fn_wasm_bindgen_765df639e0572edc___JsValue_____wasm_bindgen_765df639e0572edc___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___JsValue__core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___sys__JsNullable_wgpu_fb237351f69b1e72___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___sys__JsNullable_wgpu_fb237351f69b1e72___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true__41: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wasm_bindgen_765df639e0572edc___sys__JsNullable_wgpu_fb237351f69b1e72___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_ed718c3d60ebd546___result__Result_____wasm_bindgen_765df639e0572edc___JsError___true__42: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wgpu_fb237351f69b1e72___backend__webgpu__webgpu_sys__gen_GpuDeviceLostInfo__GpuDeviceLostInfo______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_765df639e0572edc___convert__closures_____invoke___wgpu_fb237351f69b1e72___backend__webgpu__webgpu_sys__gen_GpuDeviceLostInfo__GpuDeviceLostInfo______true__40: (a: number, b: number, c: any) => void;
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
