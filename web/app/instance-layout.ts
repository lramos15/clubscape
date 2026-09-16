import type { Tile, WorldView } from "../shared/contracts.ts";
import type { RendererInstanceLayout, RendererWorldExtensions } from "../renderer/src/index.ts";
import { AppError, deepFreeze, invariant } from "./errors.ts";
import { assetId } from "./identity.ts";
import { validateScene } from "./authority.ts";

export interface SourceInstanceChunk {
  sourceRegion: string;
  sourceOrigin: Tile;
  destinationRegion: string;
  destinationOrigin: Tile;
  quarterTurns: number;
}
export interface SourceInstanceLayout {
  chunkSize: number;
  chunks: SourceInstanceChunk[];
}
export type SourceInstanceLayouts = Record<string, SourceInstanceLayout>;

export function validateInstanceLayouts(value: SourceInstanceLayouts, regions: ReadonlySet<string>): void {
  invariant(value !== null && typeof value === "object" && !Array.isArray(value)
    && Object.keys(value).length <= 2048, "Invalid compiler-projected instance layouts.", "asset");
  const tile = (value: Tile) => value && [value.x, value.y, value.plane].every(Number.isSafeInteger)
    && value.x >= 0 && value.x <= 65535 && value.y >= 0 && value.y <= 65535 && value.plane >= 0 && value.plane <= 3;
  for (const [id, layout] of Object.entries(value)) {
    assetId(id);
    invariant(id.startsWith("instance_template.") && layout && Number.isSafeInteger(layout.chunkSize)
      && layout.chunkSize > 0 && layout.chunkSize <= 104 && Array.isArray(layout.chunks)
      && layout.chunks.length > 0 && layout.chunks.length <= 4096,
    "Invalid source instance template/chunk size.", "asset");
    const destinations = new Set<string>();
    for (const chunk of layout.chunks) {
      invariant(chunk && regions.has(chunk.sourceRegion) && regions.has(chunk.destinationRegion)
        && tile(chunk.sourceOrigin) && tile(chunk.destinationOrigin)
        && Number.isSafeInteger(chunk.quarterTurns) && chunk.quarterTurns >= 0 && chunk.quarterTurns <= 3,
      "Source instance chunk lost its declared regions, coordinates or rotation.", "asset");
      const key = `${chunk.destinationRegion}/${chunk.destinationOrigin.plane}/${chunk.destinationOrigin.x}/${chunk.destinationOrigin.y}`;
      invariant(!destinations.has(key), "Duplicate source instance destination chunk.", "asset");
      destinations.add(key);
    }
  }
}

/** The opaque instance ID proves presence only; its template must be separately supplied by authority. */
export function rendererInstanceLayout(world: WorldView, layouts: SourceInstanceLayouts | undefined,
  actualTemplate: string | null | undefined): RendererInstanceLayout | null {
  const unavailable = (message: string) => new AppError(message, {
    kind: "instance_unavailable", errorId: "renderer.instance_layout_required",
  });
  if (world.player.instance === null) {
    if (actualTemplate != null) throw unavailable("An ordinary-world snapshot cannot carry an instance template.");
    return null;
  }
  if (actualTemplate == null) throw unavailable(
    "The backend has not supplied this instance's template identity. No layout was inferred from its opaque ID, region or player tile.");
  const layout = layouts && Object.hasOwn(layouts, actualTemplate) ? layouts[actualTemplate] : undefined;
  if (!layout) throw unavailable("The actual instance template has no compiler-validated layout in this delivery.");
  if (layout.chunkSize !== 8) throw unavailable("The renderer requires original 8x8 chunk mappings; this declared template is unsupported.");
  const chunks = layout.chunks.map((chunk) => {
    if (chunk.quarterTurns !== 0) throw unavailable(
      `Turned source instance mapping is not supported: ${actualTemplate} at ${chunk.destinationOrigin.x},${chunk.destinationOrigin.y}.`);
    if ([chunk.sourceOrigin.x, chunk.sourceOrigin.y, chunk.destinationOrigin.x, chunk.destinationOrigin.y]
      .some((value) => value % layout.chunkSize !== 0)) {
      throw unavailable("Source instance origins are not aligned to the declared chunk size.");
    }
    return {
      plane: chunk.destinationOrigin.plane, chunkX: chunk.destinationOrigin.x / 8, chunkY: chunk.destinationOrigin.y / 8,
      sourcePlane: chunk.sourceOrigin.plane, sourceChunkX: chunk.sourceOrigin.x / 8, sourceChunkY: chunk.sourceOrigin.y / 8,
      quarterTurns: chunk.quarterTurns,
    };
  });
  const tile = world.player.tile;
  if (!layout.chunks.some((chunk) => chunk.destinationRegion === world.player.region
    && chunk.destinationOrigin.plane === tile.plane
    && tile.x >= chunk.destinationOrigin.x && tile.x < chunk.destinationOrigin.x + 8
    && tile.y >= chunk.destinationOrigin.y && tile.y < chunk.destinationOrigin.y + 8)) {
    throw unavailable("The authoritative player location is outside the supplied template's declared chunks.");
  }
  return deepFreeze({ template: actualTemplate, chunks });
}

export function rendererWorldView(world: WorldView, layouts: SourceInstanceLayouts | undefined,
  actualTemplate?: string | null): WorldView & RendererWorldExtensions {
  if (world.scene !== undefined) {
    validateScene(world.scene, world);
    if (actualTemplate !== undefined && actualTemplate !== world.scene.instanceTemplate) {
      throw new AppError("A supplied renderer template cannot override the authoritative scene identity.",
        { kind: "instance_unavailable", errorId: "renderer.instance_template_conflict" });
    }
    actualTemplate = world.scene.instanceTemplate;
  }
  return deepFreeze({ ...world, instanceLayout: rendererInstanceLayout(world, layouts, actualTemplate) });
}
