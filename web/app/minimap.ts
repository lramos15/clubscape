import type { MinimapSurface } from "../renderer/src/index.ts";
import { AppError, deepFreeze, invariant } from "./errors.ts";

export interface MinimapObservation {
  baseX: number; baseY: number; plane: number; revision: number;
  width: number; height: number; scale: number; marginX: number; marginY: number;
  complete: boolean; notes: string[]; stats: MinimapSurface["stats"]; icons: MinimapSurface["icons"];
  delivered: boolean; fullSurfaceFidelityAccepted: false;
}

/** Actual source surface relay only. No static raster, icon inference, scaling or readiness grant. */
export class MinimapRelay {
  #key: string | null = null;
  #state: Readonly<MinimapObservation> | null = null;
  #reported = false;
  #apply: ((surface: MinimapSurface) => void) | null;
  #report: (error: AppError) => void;

  constructor(apply: ((surface: MinimapSurface) => void) | null, report: (error: AppError) => void) {
    this.#apply = apply; this.#report = report;
  }

  update(surface: MinimapSurface, deviceEpoch: string): void {
    invariant(surface.width === 512 && surface.height === 512 && surface.scale === 4
      && surface.marginX === 48 && surface.marginY === 48
      && surface.pixels.width === surface.width && surface.pixels.height === surface.height
      && surface.pixels.data.length === 512 * 512 * 4 && surface.mask.length === 512 * 512
      && Number.isSafeInteger(surface.revision) && surface.revision >= 0
      && [surface.baseX, surface.baseY, surface.plane].every(Number.isSafeInteger)
      && surface.plane >= 0 && surface.plane <= 3,
    "The renderer supplied an invalid native minimap surface.", "renderer");
    const key = `${deviceEpoch}/${surface.baseX}/${surface.baseY}/${surface.plane}/${surface.revision}`;
    if (key === this.#key) return;
    if (this.#apply) this.#apply(surface);
    else if (!this.#reported) {
      this.#reported = true;
      this.#report(new AppError("The real renderer minimap surface is available, but the UI has no relayed live-surface/icon setter. Its static minimap is not dynamic-surface fidelity.", { kind: "minimap_integration" }));
    }
    this.#state = deepFreeze({
      baseX: surface.baseX, baseY: surface.baseY, plane: surface.plane, revision: surface.revision,
      width: surface.width, height: surface.height, scale: surface.scale, marginX: surface.marginX, marginY: surface.marginY,
      complete: surface.complete, notes: [...surface.notes], stats: { ...surface.stats },
      icons: surface.icons.map((icon) => ({ ...icon })), delivered: this.#apply !== null, fullSurfaceFidelityAccepted: false,
    });
    this.#key = key;
  }

  observe(): Readonly<MinimapObservation> | null { return this.#state; }
}
