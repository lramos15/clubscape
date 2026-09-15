import { fullHudViewport } from "../renderer/src/index.ts";
import { deepFreeze, invariant } from "./errors.ts";

export interface FullHudSurface {
  logicalWidth: number;
  logicalHeight: number;
  deviceScaleFactor: number;
  width: number;
  height: number;
  zoom: number;
}
export interface SurfaceResize {
  world(width: number, height: number): void;
  camera(zoom: number): void;
  ui(width: number, height: number): void;
  observe(width: number, height: number, scale: number): void;
}

/** Native projection and resize deduplication only; HUD anchors and camera position remain source-owned. */
export function resizeFullHud(previous: Readonly<FullHudSurface> | null, logicalWidth: number, logicalHeight: number,
  deviceScaleFactor: number, apply: SurfaceResize): Readonly<FullHudSurface> {
  invariant([logicalWidth, logicalHeight].every((value) => Number.isSafeInteger(value) && value > 0)
    && Number.isFinite(deviceScaleFactor) && deviceScaleFactor > 0,
  "The full-HUD viewport dimensions are invalid.", "viewport");
  if (previous?.logicalWidth === logicalWidth && previous.logicalHeight === logicalHeight
    && previous.deviceScaleFactor === deviceScaleFactor) return previous;
  const width = Math.round(logicalWidth * deviceScaleFactor), height = Math.round(logicalHeight * deviceScaleFactor);
  const viewport = fullHudViewport(width, height);
  invariant(viewport.zoom > 0 && viewport.x === 0 && viewport.y === 0
    && viewport.width === width && viewport.height === height,
  "This canvas needs an unsupported native letterbox/tiny-viewport composition; no projection substitute was applied.", "viewport");
  if (previous?.width !== width || previous.height !== height) {
    apply.world(width, height);
    apply.camera(viewport.zoom);
  }
  if (previous?.logicalWidth !== logicalWidth || previous.logicalHeight !== logicalHeight) apply.ui(logicalWidth, logicalHeight);
  apply.observe(logicalWidth, logicalHeight, deviceScaleFactor);
  return deepFreeze({ logicalWidth, logicalHeight, deviceScaleFactor, width, height, zoom: viewport.zoom });
}
