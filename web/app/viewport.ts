import { fullHudViewport } from "../renderer/src/index.ts";
import { deepFreeze, invariant } from "./errors.ts";

export interface FullHudSurface {
  logicalWidth: number;
  logicalHeight: number;
  deviceScaleFactor: number;
  width: number;
  height: number;
  zoom: number | null;
}
export interface SurfaceResize {
  world(width: number, height: number): void;
  camera(zoom: number): void;
  nativeCamera?(width: number, height: number): void;
  ui(width: number, height: number): void;
  observe(width: number, height: number, scale: number): void;
}

/** Native projection and resize deduplication only; HUD anchors and camera position remain source-owned. */
export function resizeFullHud(previous: Readonly<FullHudSurface> | null, logicalWidth: number, logicalHeight: number,
  deviceScaleFactor: number, apply: SurfaceResize, projection: "recorded-full-hud" | "native-controller" = "recorded-full-hud"): Readonly<FullHudSurface> {
  invariant([logicalWidth, logicalHeight].every((value) => Number.isSafeInteger(value) && value > 0)
    && Number.isFinite(deviceScaleFactor) && deviceScaleFactor > 0,
  "The full-HUD viewport dimensions are invalid.", "viewport");
  const sameProjection = previous !== null && (projection === "native-controller" ? previous.zoom === null : previous.zoom !== null);
  if (previous?.logicalWidth === logicalWidth && previous.logicalHeight === logicalHeight
    && previous.deviceScaleFactor === deviceScaleFactor && sameProjection) return previous;
  const width = Math.round(logicalWidth * deviceScaleFactor), height = Math.round(logicalHeight * deviceScaleFactor);
  const viewport = projection === "recorded-full-hud" ? fullHudViewport(width, height) : null;
  invariant(width > 0 && height > 0 && width <= 32767 && height <= 32767
    && (viewport === null || viewport.zoom > 0 && viewport.x === 0 && viewport.y === 0
    && viewport.width === width && viewport.height === height),
  "This canvas needs an unsupported native letterbox/tiny-viewport composition; no projection substitute was applied.", "viewport");
  invariant(projection !== "native-controller" || apply.nativeCamera !== undefined,
    "Native camera resizing requires its real Rust controller.", "camera_unavailable");
  const changedPixels = previous?.width !== width || previous.height !== height;
  if (changedPixels || !sameProjection) {
    if (projection === "native-controller") apply.nativeCamera!(width, height);
    if (changedPixels) apply.world(width, height);
    if (viewport !== null) apply.camera(viewport.zoom);
  }
  if (previous?.logicalWidth !== logicalWidth || previous.logicalHeight !== logicalHeight) apply.ui(logicalWidth, logicalHeight);
  apply.observe(logicalWidth, logicalHeight, deviceScaleFactor);
  return deepFreeze({ logicalWidth, logicalHeight, deviceScaleFactor, width, height, zoom: viewport?.zoom ?? null });
}
