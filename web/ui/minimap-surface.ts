export interface UiMinimapIcon { x: number; y: number; plane: number; element: number }
export interface UiMinimapSurface {
  width: number; height: number; scale: number; marginX: number; marginY: number;
  baseX: number; baseY: number; plane: number; revision: number; complete: boolean;
  stats: { terrainTiles: number; wallMarks: number; diagonalMarks: number; mapScenes: number; unresolved: number };
  notes: string[]; icons: UiMinimapIcon[]; pixels: ImageData; mask: Uint8Array;
}
export interface UiMinimapStatus extends Omit<UiMinimapSurface, "pixels" | "mask"> {
  sourceMaskPixels: number;
  missingElements: number[];
  uiIssues: string[];
}
export class UiMinimapError extends Error {
  readonly errorId: string;
  constructor(message: string, errorId = "ui.minimap.invalid") {
    super(message); this.name = "UiMinimapError"; this.errorId = errorId;
  }
}
const coordinate = (value: number) => Number.isSafeInteger(value);
const nonnegative = (value: number) => coordinate(value) && value >= 0;
const plane = (value: number) => coordinate(value) && value >= 0 && value <= 3;

export function minimapSurfaceProblem(value: UiMinimapSurface): string | null {
  if (!value || typeof value !== "object") return "A renderer minimap surface is required.";
  if (value.width !== 512 || value.height !== 512 || value.scale !== 4 || value.marginX !== 48 || value.marginY !== 48)
    return "The minimap must use the original 512x512, scale4, margin48 format.";
  if (!coordinate(value.baseX) || !coordinate(value.baseY) || !plane(value.plane) || !nonnegative(value.revision))
    return "The minimap base, plane or revision is invalid.";
  if (typeof value.complete !== "boolean" || !Array.isArray(value.notes) || value.notes.some(note => typeof note !== "string"))
    return "The renderer must supply its actual minimap completeness and notes.";
  if (!value.stats || ![value.stats.terrainTiles, value.stats.wallMarks, value.stats.diagonalMarks,
    value.stats.mapScenes, value.stats.unresolved].every(nonnegative)) return "The renderer minimap statistics are invalid.";
  if (!value.pixels || value.pixels.width !== value.width || value.pixels.height !== value.height ||
      !(value.pixels.data instanceof Uint8ClampedArray) || value.pixels.data.length !== value.width * value.height * 4)
    return "The renderer minimap requires complete RGBA8 pixel data.";
  if (!(value.mask instanceof Uint8Array) || value.mask.length !== value.width * value.height)
    return "The renderer minimap requires its complete original source mask.";
  for (let pixel = 0; pixel < value.mask.length; pixel++) {
    if (value.mask[pixel] !== 0 && value.mask[pixel] !== 1) return "The minimap source mask must contain only 0 or 1.";
    if (value.pixels.data[pixel * 4 + 3] !== 255) return "The original renderer minimap is opaque RGBA8.";
  }
  if (!Array.isArray(value.icons) || value.icons.some(icon => !icon || !coordinate(icon.x) || !coordinate(icon.y) ||
      !plane(icon.plane) || !nonnegative(icon.element))) return "The renderer minimap icon identities or positions are invalid.";
  return null;
}

function metadata(value: UiMinimapSurface): Omit<UiMinimapSurface, "pixels" | "mask"> {
  return {
    width: value.width, height: value.height, scale: value.scale, marginX: value.marginX, marginY: value.marginY,
    baseX: value.baseX, baseY: value.baseY, plane: value.plane, revision: value.revision, complete: value.complete,
    stats: { ...value.stats }, notes: [...value.notes], icons: value.icons.map(icon => ({ ...icon })),
  };
}
function equalBytes(a: Uint8Array | Uint8ClampedArray, b: Uint8Array | Uint8ClampedArray): boolean {
  return a.length === b.length && a.every((value, index) => value === b[index]);
}

/** Copies each supplied revision so later WASM writes cannot mutate an already displayed frame. */
export class MinimapSurfaceStore {
  private frame: UiMinimapSurface | null = null;
  private scope: string | null = null;

  current(): UiMinimapSurface | null { return this.frame; }
  clear(): void { this.frame = null; this.scope = null; }
  retainScope(scope: string | null): void { if (this.scope !== scope) this.clear(); }
  set(value: UiMinimapSurface, scope: string): boolean {
    const problem = minimapSurfaceProblem(value);
    if (problem) throw new UiMinimapError(problem);
    const previous = this.scope === scope ? this.frame : null;
    if (previous && value.revision < previous.revision)
      throw new UiMinimapError("The renderer supplied an older minimap revision.", "ui.minimap.stale");
    if (previous && value.revision === previous.revision) {
      if (JSON.stringify(metadata(value)) !== JSON.stringify(metadata(previous)) ||
          !equalBytes(value.pixels.data, previous.pixels.data) || !equalBytes(value.mask, previous.mask))
        throw new UiMinimapError("A minimap revision was reused for different data.", "ui.minimap.revision_reused");
      return false;
    }
    const pixels = new ImageData(value.width, value.height);
    pixels.data.set(value.pixels.data);
    this.frame = { ...metadata(value), pixels, mask: value.mask.slice() };
    this.scope = scope;
    return true;
  }
  status(missingElements: readonly number[] = [], uiIssues: readonly string[] = []): UiMinimapStatus | null {
    return this.frame ? {
      ...metadata(this.frame), sourceMaskPixels: this.frame.mask.reduce((sum, value) => sum + value, 0),
      missingElements: [...missingElements], uiIssues: [...uiIssues],
    } : null;
  }
}
