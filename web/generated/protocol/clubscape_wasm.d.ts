/* tslint:disable */
/* eslint-disable */

/**
 * The sole JS/WASM protocol boundary. No credentials or leases occur in state().
 */
export class BrowserClient {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Transport-only, memory-only. Never copy into UI state, storage, logs or URLs.
     */
    authorization(): string | undefined;
    constructor();
    prepare(request_id: string, operation: string, input: string): Uint8Array;
    receive(bytes: Uint8Array): string;
    receive_for(request: string, bytes: Uint8Array): string;
    request_id(bytes: Uint8Array): string;
    retry_lifecycle(): Uint8Array | undefined;
    retry_uncertain_input(): Uint8Array | undefined;
    set_catalog(input: string): string;
    state(): string;
    submit(request_id: string, input: string): Uint8Array;
    submit_selected(request_id: string, input: string, item_id: string): Uint8Array;
    transport_lost(): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_browserclient_free: (a: number, b: number) => void;
    readonly browserclient_authorization: (a: number) => [number, number];
    readonly browserclient_new: () => number;
    readonly browserclient_prepare: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number, number];
    readonly browserclient_receive: (a: number, b: number, c: number) => [number, number, number, number];
    readonly browserclient_receive_for: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly browserclient_request_id: (a: number, b: number, c: number) => [number, number, number, number];
    readonly browserclient_retry_lifecycle: (a: number) => [number, number, number, number];
    readonly browserclient_retry_uncertain_input: (a: number) => [number, number, number, number];
    readonly browserclient_set_catalog: (a: number, b: number, c: number) => [number, number, number, number];
    readonly browserclient_state: (a: number) => [number, number, number, number];
    readonly browserclient_submit: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly browserclient_submit_selected: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number, number];
    readonly browserclient_transport_lost: (a: number) => void;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
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
