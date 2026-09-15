import type { MapIconSprite, MinimapSurface } from "../renderer/src/index.ts";
import { AppError, deepFreeze, invariant } from "./errors.ts";

export interface MinimapObservation {
  baseX: number; baseY: number; plane: number; revision: number;
  width: number; height: number; scale: number; marginX: number; marginY: number;
  complete: boolean; notes: string[]; stats: MinimapSurface["stats"]; icons: MinimapSurface["icons"];
  sourceIconMismatches: number | null;
  iconSprites: { available: boolean; count: number; bytes: number; delivered: boolean };
  iconProjection: "awaiting-exact-unit-helper";
  delivered: boolean; fullSurfaceFidelityAccepted: false;
}

/** Actual source surface relay only. No static raster, icon inference, scaling or readiness grant. */
export class MinimapRelay {
  #key: string | null = null;
  #state: Readonly<MinimapObservation> | null = null;
  #reported = false;
  #apply: ((surface: MinimapSurface) => void) | null;
  #report: (error: AppError) => void;
  #applyIcons: ((sprites: ReadonlyMap<number, MapIconSprite>) => void) | null;
  #icons: ReadonlyMap<number, MapIconSprite> | null = null;
  #iconEpoch = "";
  #iconState = { available: false, count: 0, bytes: 0, delivered: false };
  #iconsReported = false;

  constructor(apply: ((surface: MinimapSurface) => void) | null, report: (error: AppError) => void,
    applyIcons: ((sprites: ReadonlyMap<number, MapIconSprite>) => void) | null = null) {
    this.#apply = apply; this.#report = report; this.#applyIcons = applyIcons;
  }

  update(surface: MinimapSurface, deviceEpoch: string, sprites?: ReadonlyMap<number, MapIconSprite>): void {
    invariant(surface.width === 512 && surface.height === 512 && surface.scale === 4
      && surface.marginX === 48 && surface.marginY === 48
      && surface.pixels.width === surface.width && surface.pixels.height === surface.height
      && surface.pixels.data.length === 512 * 512 * 4 && surface.mask.length === 512 * 512
      && Number.isSafeInteger(surface.revision) && surface.revision >= 0
      && [surface.baseX, surface.baseY, surface.plane].every(Number.isSafeInteger)
      && surface.plane >= 0 && surface.plane <= 3
      && (surface.sourceIconMismatches === null || (Number.isSafeInteger(surface.sourceIconMismatches) && surface.sourceIconMismatches >= 0)),
    "The renderer supplied an invalid native minimap surface.", "renderer");
    const iconsChanged = this.#supplyIcons(sprites, deviceEpoch);
    const key = `${deviceEpoch}/${surface.baseX}/${surface.baseY}/${surface.plane}/${surface.revision}`;
    if (key === this.#key && !iconsChanged) return;
    if (this.#apply && key !== this.#key) this.#apply(surface);
    else if (!this.#apply && !this.#reported) {
      this.#reported = true;
      this.#report(new AppError("The real renderer minimap surface is available, but the UI has no relayed live-surface/icon setter. Its static minimap is not dynamic-surface fidelity.", { kind: "minimap_integration" }));
    }
    this.#state = deepFreeze({
      baseX: surface.baseX, baseY: surface.baseY, plane: surface.plane, revision: surface.revision,
      width: surface.width, height: surface.height, scale: surface.scale, marginX: surface.marginX, marginY: surface.marginY,
      complete: surface.complete, notes: [...surface.notes], stats: { ...surface.stats },
      icons: surface.icons.map((icon) => ({ ...icon })), sourceIconMismatches: surface.sourceIconMismatches,
      iconSprites: { ...this.#iconState }, iconProjection: "awaiting-exact-unit-helper",
      delivered: this.#apply !== null, fullSurfaceFidelityAccepted: false,
    });
    this.#key = key;
  }

  #supplyIcons(sprites: ReadonlyMap<number, MapIconSprite> | undefined, epoch: string): boolean {
    if (sprites === this.#icons && epoch === this.#iconEpoch) return false;
    if (sprites === undefined || sprites.size === 0) {
      const changed = this.#iconState.available || this.#iconEpoch !== epoch;
      this.#icons = sprites ?? null; this.#iconEpoch = epoch;
      this.#iconState = { available: false, count: 0, bytes: 0, delivered: false };
      return changed;
    }
    invariant(sprites instanceof Map && sprites.size <= 4096, "Invalid original map-icon sprite table.", "renderer");
    let bytes = 0;
    for (const [element, sprite] of sprites) {
      invariant(Number.isSafeInteger(element) && element >= 0 && sprite.element === element
        && [sprite.width, sprite.height, sprite.maxWidth, sprite.maxHeight, sprite.offsetX, sprite.offsetY, sprite.category].every(Number.isSafeInteger)
        && sprite.width >= 0 && sprite.height >= 0 && sprite.width <= 4096 && sprite.height <= 4096
        && sprite.pixels.width === Math.max(1, sprite.width) && sprite.pixels.height === Math.max(1, sprite.height)
        && sprite.pixels.data.length === sprite.pixels.width * sprite.pixels.height * 4,
      "The renderer supplied invalid original map-icon pixels or metadata.", "renderer");
      bytes += sprite.pixels.data.length;
    }
    invariant(bytes <= 64 * 1024 * 1024, "Original map-icon pixels exceed the bounded UI delivery size.", "renderer");
    if (this.#applyIcons) this.#applyIcons(new Map(sprites));
    else if (!this.#iconsReported) {
      this.#iconsReported = true;
      this.#report(new AppError("Original map-icon sprites are decoded, but the UI icon sink and exact native offset-unit helper are not integrated. No world128 radius or clipping rule was guessed.",
        { kind: "minimap_integration", errorId: "renderer.minimap_icon_integration_required" }));
    }
    this.#icons = sprites; this.#iconEpoch = epoch;
    this.#iconState = { available: true, count: sprites.size, bytes, delivered: this.#applyIcons !== null };
    return true;
  }

  observe(): Readonly<MinimapObservation> | null { return this.#state; }
}
