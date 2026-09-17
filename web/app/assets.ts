import type { ClientAssets } from "../shared/contracts.ts";
import { AppError, deepFreeze, invariant } from "./errors.ts";
import { sha256 } from "./identity.ts";
import type { AssetRecord, ContentManifest } from "./manifest.ts";
import { boundedBytes } from "./transport.ts";
import type { Fetch } from "./transport.ts";

export interface AssetObservation {
  id: string; sha256: string; fetched: boolean; decoded: boolean;
  bytes: number; fetchMs: number | null; decodeMs: number | null;
}

export class AssetLoader implements ClientAssets {
  readonly baseUrl = "/";
  readonly manifest: ContentManifest;
  readonly manifestSha256: string;
  #entries: Map<string, AssetRecord>;
  #aliases: Map<string, string>;
  #observations = new Map<string, AssetObservation>();
  #bytes = new Map<string, Promise<Uint8Array<ArrayBuffer>>>();
  #images = new Map<string, Promise<HTMLImageElement>>();
  #json = new Map<string, Promise<unknown>>();
  #fetch: Fetch;
  #changed: () => void;
  #controller = new AbortController();

  constructor(manifest: ContentManifest, manifestSha256: string, options: { fetch?: Fetch; changed?: () => void } = {}) {
    this.manifest = manifest;
    this.manifestSha256 = manifestSha256;
    this.#entries = new Map(manifest.assets.map((asset) => [asset.id, asset]));
    this.#aliases = new Map(Object.entries(manifest.aliases ?? {}));
    this.#fetch = options.fetch ?? globalThis.fetch.bind(globalThis);
    this.#changed = options.changed ?? (() => {});
  }

  url(id: string): string {
    return this.#entry(id).url;
  }

  #entry(id: string): AssetRecord {
    const entry = this.#entries.get(this.#aliases.get(id) ?? id);
    if (!entry) throw new AppError(`Required source asset is unavailable: ${id}.`, { kind: "asset" });
    return entry;
  }

  async bytes(id: string): Promise<Uint8Array<ArrayBuffer>> {
    id = this.#entry(id).id;
    let promise = this.#bytes.get(id);
    if (!promise) {
      promise = this.#loadBytes(id);
      this.#bytes.set(id, promise);
      promise.catch(() => this.#bytes.delete(id));
    }
    return new Uint8Array(await promise);
  }

  async #loadBytes(id: string): Promise<Uint8Array<ArrayBuffer>> {
    const asset = this.#entry(id);
    const before = performance.now();
    this.#observations.set(id, {
      id, sha256: asset.sha256, fetched: false, decoded: false, bytes: 0, fetchMs: null, decodeMs: null,
    });
    this.#changed();
    const timeout = AbortSignal.timeout(30_000);
    try {
      const response = await this.#fetch(asset.url, {
        mode: "same-origin", credentials: "omit", redirect: "error", referrerPolicy: "no-referrer",
        signal: AbortSignal.any([this.#controller.signal, timeout]),
      });
      invariant(response.ok && !response.redirected, `Source asset fetch failed (${response.status}): ${id}.`);
      invariant(response.headers.get("content-type")?.split(";")[0]?.trim() === asset.contentType.split(";")[0],
        `Source asset MIME mismatch: ${id}.`);
      const bytes = await boundedBytes(response, asset.bytes);
      invariant(bytes.byteLength === asset.bytes && await sha256(bytes) === asset.sha256, `Source asset hash/length mismatch: ${id}.`);
      this.#observations.set(id, {
        id, sha256: asset.sha256, fetched: true, decoded: false, bytes: bytes.byteLength,
        fetchMs: performance.now() - before, decodeMs: null,
      });
      this.#changed();
      return bytes;
    } catch (error) {
      if (error instanceof AppError) throw error;
      throw new AppError(`Source asset could not be fetched: ${id}.`, { kind: "asset" });
    }
  }

  async json(id: string): Promise<unknown> {
    id = this.#entry(id).id;
    let promise = this.#json.get(id);
    if (!promise) {
      promise = (async () => {
        invariant(this.#entry(id).contentType.startsWith("application/json"), `Source asset is not JSON: ${id}.`);
        const bytes = await this.bytes(id);
        const before = performance.now();
        let result: unknown;
        try { result = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); }
        catch { throw new AppError(`Source JSON could not be decoded: ${id}.`, { kind: "asset" }); }
        this.#decoded(id, before);
        return deepFreeze(result);
      })();
      this.#json.set(id, promise);
      promise.catch(() => this.#json.delete(id));
    }
    return promise;
  }

  async decode<T>(id: string, decoder: (bytes: Uint8Array<ArrayBuffer>) => Promise<T>): Promise<T> {
    id = this.#entry(id).id;
    const bytes = await this.bytes(id);
    const before = performance.now();
    const result = await decoder(bytes);
    this.#decoded(id, before);
    return result;
  }

  retain(ids: readonly string[]): void {
    const keep = new Set(ids.map((id) => this.#entry(id).id));
    for (const id of keep) this.#entry(id);
    for (const id of this.#bytes.keys()) {
      if (!keep.has(id)) {
        this.#bytes.delete(id);
        this.#images.delete(id);
        this.#json.delete(id);
        this.#observations.delete(id);
      }
    }
    this.#changed();
  }

  async image(id: string): Promise<HTMLImageElement> {
    id = this.#entry(id).id;
    let promise = this.#images.get(id);
    if (!promise) {
      promise = (async () => {
        const entry = this.#entry(id);
        invariant(/^image\/(png|jpeg|webp)$/.test(entry.contentType), `Source asset is not a supported raster: ${id}.`);
        const bytes = await this.bytes(id);
        const before = performance.now();
        const image = new Image();
        // The server's img-src policy allows data:, not blob: URLs.
        let binary = "";
        for (let offset = 0; offset < bytes.length; offset += 8192) {
          binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
        }
        image.src = `data:${entry.contentType};base64,${btoa(binary)}`;
        try { await image.decode(); }
        catch { throw new AppError(`Source raster could not be decoded: ${id}.`, { kind: "asset" }); }
        invariant(image.naturalWidth > 0 && image.naturalHeight > 0
          && image.naturalWidth * image.naturalHeight <= 16_777_216, `Source raster dimensions are invalid: ${id}.`);
        this.#decoded(id, before);
        return image;
      })();
      this.#images.set(id, promise);
      promise.catch(() => this.#images.delete(id));
    }
    return promise;
  }

  #decoded(id: string, before: number): void {
    const observation = this.#observations.get(id);
    invariant(observation?.fetched, "Asset decode has no verified fetch.");
    this.#observations.set(id, { ...observation, decoded: true, decodeMs: performance.now() - before });
    this.#changed();
  }

  async preload(ids: readonly string[]): Promise<void> {
    let next = 0;
    const workers = Array.from({ length: Math.min(4, ids.length) }, async () => {
      for (;;) {
        const id = ids[next++];
        if (id === undefined) return;
        const mime = this.#entry(id).contentType;
        if (mime.startsWith("image/")) await this.image(id);
        else if (mime.startsWith("application/json")) await this.json(id);
        else await this.bytes(id);
      }
    });
    await Promise.all(workers);
  }

  observe(): AssetObservation[] {
    return Array.from(this.#observations.values(), (value) => ({ ...value }));
  }

  counts(ids: readonly string[]): { fetched: number; decoded: number; total: number } {
    const states = ids.map((id) => this.#observations.get(this.#entry(id).id));
    return {
      total: ids.length, fetched: states.filter((state) => state?.fetched).length,
      decoded: states.filter((state) => state?.decoded).length,
    };
  }

  dispose(): void {
    this.#controller.abort();
    this.#bytes.clear();
    this.#images.clear();
    this.#json.clear();
    this.#observations.clear();
  }
}
