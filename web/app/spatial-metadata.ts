import { AUDIO_INPUTS, sourceObjectBounds, sourceObjectDefinition } from "../audio/index.ts";
import type { SourceAudioScene, SourceSceneEmitter, SourceWorldOwner } from "../audio/index.ts";
import type { GameplayUiView, WorldView } from "../shared/contracts.ts";
import { SOURCE_PACK_SHA256 } from "../shared/contracts.ts";
import type { ScenePlacement } from "../renderer/src/index.ts";
import type { AssetLoader } from "./assets.ts";
import type { AssetRecord, ContentManifest } from "./manifest.ts";
import { sourceSceneAuthority, validateScene } from "./authority.ts";
import { AppError, deepFreeze } from "./errors.ts";
import { canonicalJson } from "./identity.ts";
import { rendererInstanceLayout } from "./instance-layout.ts";

export interface SpatialGeometry {
  sceneId: string | null;
  placement: ScenePlacement | null;
  loadedSquares: readonly number[] | null;
  /** Supplied by the host view. Root-renderer null is not inferred from a gameplay instance ID. */
  owner: SourceWorldOwner | null;
}

export type SpatialWorld = Pick<WorldView, "scene" | "dynamicObjects" | "audioAuthority"> & {
  player: Pick<WorldView["player"], "id" | "region" | "tile" | "instance">;
  ui?: { version: 1; appearance: Pick<GameplayUiView["appearance"], "base"> };
};

interface Placement {
  id: string;
  objectId: number;
  x: number;
  y: number;
  plane: number;
  type: number;
  orientation: number;
}

type MetadataAssets = Pick<AssetLoader, "manifest" | "json" | "decode" | "preload">;
const columns = ["object_id", "world_x", "world_y", "plane", "type", "orientation"];

function missing(message: string, field: string): never {
  throw new AppError(message, { kind: "source_spatial_metadata", errorId: `audio.source.${field}` });
}
function record(value: unknown, field: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) missing(`Invalid original ${field} metadata.`, "metadata_shape");
  return value as Record<string, unknown>;
}
function list(value: unknown, field: string): unknown[] {
  if (!Array.isArray(value)) missing(`Missing original ${field} metadata.`, "metadata_shape");
  return value;
}
function integer(value: unknown, minimum: number, maximum: number, field: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) {
    missing(`Invalid original ${field} value.`, "metadata_shape");
  }
  return value;
}
function inside(x: number, y: number, scene: ScenePlacement): boolean {
  return x >= scene.baseX && y >= scene.baseY && x < scene.baseX + scene.sizeTiles && y < scene.baseY + scene.sizeTiles;
}

/** Original placement metadata only. Native audio still owns morphs, distance, gain and visibility. */
export class SourceSpatialMetadata {
  readonly requiredAssets: readonly string[];
  #manifest: ContentManifest;
  #regions: ReadonlyMap<number, readonly Placement[]>;
  #ambient: ReadonlySet<number>;
  #nonAmbient: ReadonlySet<number>;
  #bodyNpc: number;

  private constructor(manifest: ContentManifest, regions: Map<number, readonly Placement[]>,
    ambient: Set<number>, nonAmbient: Set<number>, bodyNpc: number, assets: Set<string>) {
    this.#manifest = manifest;
    this.#regions = regions;
    this.#ambient = ambient;
    this.#nonAmbient = nonAmbient;
    this.#bodyNpc = bodyNpc;
    this.requiredAssets = Object.freeze([...assets]);
  }

  static async load(assets: MetadataAssets, requireAsset: (id: string) => void = () => {}): Promise<SourceSpatialMetadata> {
    const manifest = assets.manifest;
    if (manifest.sourcePackSha256 !== SOURCE_PACK_SHA256 || manifest.renderer?.coverage !== "source_world_blocks"
      || !manifest.rendererManifest) missing("Spatial metadata requires the actual source-world renderer delivery.", "renderer_metadata");
    const rendererDelivery = manifest.renderer;
    const touched = new Set<string>();
    const declared = (id: string): AssetRecord => {
      const resolved = manifest.aliases?.[id] ?? id;
      const value = manifest.assets.find(asset => asset.id === resolved);
      if (!value) missing(`Required original metadata asset is not public: ${id}.`, "metadata_asset");
      if (value.contentType.split(";")[0]?.trim() !== "application/json") {
        missing(`Original metadata asset is not declared JSON: ${id}.`, "metadata_asset");
      }
      if (!touched.has(value.id)) { touched.add(value.id); requireAsset(value.id); }
      return value;
    };
    const renderAsset = declared(manifest.rendererManifest);
    if (renderAsset.sha256 !== rendererDelivery.manifestSha256) missing("Renderer metadata identity disagrees with its delivery.", "renderer_identity");
    const mapAsset = declared(AUDIO_INPUTS.map.path);
    if (mapAsset.sha256 !== AUDIO_INPUTS.map.sha256 || mapAsset.bytes !== AUDIO_INPUTS.map.bytes) {
      missing("The original ambient-object source-map pin changed.", "catalog_identity");
    }
    const render = record(await assets.json(renderAsset.id), "renderer");
    if (render.approved_reference_pack_sha256 !== SOURCE_PACK_SHA256 || render.units_per_tile !== 128) {
      missing("The renderer does not declare the source128-unit coordinate ABI.", "coordinate_contract");
    }
    const bodyNpc = integer(record(render.gear_pose_fits, "player pose table").body_npc, 0, 0xffffffff, "player source NPC");
    const body = list(render.npc_definitions, "NPC definitions").map(value => record(value, "NPC definition"))
      .find(value => value.npc_id === bodyNpc);
    if (!body || body.size !== 1) missing("The renderer's current player footprint is not the declared one-tile body.", "coordinate_contract");
    const map = record(await assets.json(mapAsset.id), "audio source map");
    if (map.schema_version !== 1) missing("Unsupported original audio source-map version.", "catalog_identity");
    const ambient = new Set<number>();
    for (const value of list(map.ambient_objects, "ambient object catalogue")) {
      const entry = record(value, "ambient object");
      const id = integer(entry.object_id, 0, 0xffffffff, "ambient object ID");
      if (ambient.has(id) || !sourceObjectDefinition(id)) missing(`Original ambient object ${id} lacks its unique native calibration.`, "object_calibration");
      ambient.add(id);
    }
    const blocks = new Map(list(render.blocks, "source blocks").map(value => {
      const block = record(value, "source block");
      return [integer(block.square, 0, 65535, "map square"), block] as const;
    }));
    if (blocks.size !== list(render.blocks, "source blocks").length) missing("Duplicate renderer square metadata.", "region_identity");
    const sources = Object.entries(manifest.regions).map(([id, region]) => {
      const square = rendererDelivery.regions[id]?.square;
      if (square === undefined) missing(`No original renderer-square identity for ${id}.`, "region_identity");
      return { square, asset: declared(region.sceneAsset) };
    });
    await assets.preload(sources.map(value => value.asset.id));
    const regions = new Map<number, readonly Placement[]>();
    for (const { square, asset } of sources) {
      if (regions.has(square)) missing(`Duplicate original placement source for square ${square}.`, "region_identity");
      const block = blocks.get(square);
      if (!block) missing(`Square ${square} has no published block metadata.`, "region_identity");
      const placements = await assets.decode(asset.id, async bytes => {
        let parsed: unknown;
        try { parsed = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)); }
        catch { missing(`Original placement JSON could not be decoded: ${asset.id}.`, "region_bytes"); }
        const data = record(parsed, "region");
        if (data.schema_version !== 1 || data.region_id !== square
          || data.base_x !== block.origin_x || data.base_y !== block.origin_y
          || canonicalJson(data.dimensions) !== "[4,64,64]"
          || canonicalJson(data.placement_columns) !== canonicalJson(columns)) {
          missing(`Original region columns, dimensions or identity do not match square ${square}: ${asset.id}.`, "region_identity");
        }
        const baseX = integer(data.base_x, 0, 16383, "region base x");
        const baseY = integer(data.base_y, 0, 16383, "region base y");
        return list(data.placements, "placement rows").map((value, index): Placement => {
          const row = list(value, "placement row");
          if (row.length !== columns.length) missing(`Malformed placement ${index} in ${asset.id}.`, "region_row");
          return Object.freeze({
            id: `${asset.id}:placement:${index}`,
            objectId: integer(row[0], 0, 0xffffffff, "placed object ID"),
            x: integer(row[1], baseX, baseX + 63, "placed world x"),
            y: integer(row[2], baseY, baseY + 63, "placed world y"),
            plane: integer(row[3], 0, 3, "placed plane"),
            type: integer(row[4], 0, 22, "placement type"),
            orientation: integer(row[5], 0, 3, "placement orientation"),
          });
        });
      });
      regions.set(square, Object.freeze(placements));
    }

    const objectAssets = new Map<number, string>();
    for (const [id, value] of Object.entries(manifest.catalog.entities)) {
      if (!id.startsWith("object.") || value.sourceId === null || value.asset === null) continue;
      const previous = objectAssets.get(value.sourceId);
      if (previous !== undefined && previous !== value.asset) missing(`Ambiguous original object asset for ${value.sourceId}.`, "object_identity");
      objectAssets.set(value.sourceId, value.asset);
    }
    const nonAmbient = new Set<number>(), visiting = new Set<number>();
    // This proves absence across every source transform target; it never selects a varp value.
    const noAmbient = async (id: number): Promise<boolean> => {
      if (sourceObjectDefinition(id)) return false;
      if (nonAmbient.has(id)) return true;
      if (visiting.has(id)) missing(`Cyclic original sound metadata for object ${id}.`, "object_identity");
      const assetId = objectAssets.get(id);
      if (!assetId) missing(`Original sound metadata for object ${id} is not delivered.`, "object_metadata");
      const asset = declared(assetId);
      visiting.add(id);
      const data = record(await assets.json(asset.id), "object sound definition");
      if (data.id !== id) missing(`Object metadata ${asset.id} does not identify ${id}.`, "object_identity");
      const sound = integer(data.ambientSoundId, -1, 65534, "object ambient sound");
      const random = data.ambientSoundIds === null ? [] : list(data.ambientSoundIds, "object random sounds");
      if (sound !== -1 || random.length !== 0) {
        missing(`Object ${id} has original sound data but no native source calibration.`, "object_calibration");
      }
      const transforms = data.configChangeDest === null ? [] : list(data.configChangeDest, "object transform targets");
      for (const value of transforms) {
        const target = integer(value, -1, 0xffffffff, "object transform target");
        if (target !== -1 && !await noAmbient(target)) {
          missing(`Object ${id} can transform to audible ${target} without a calibrated root binding.`, "object_calibration");
        }
      }
      visiting.delete(id);
      nonAmbient.add(id);
      return true;
    };
    for (const value of list(render.dynamic_objects, "dynamic object bindings")) {
      await noAmbient(integer(record(value, "dynamic object binding").object_id, 0, 0xffffffff, "dynamic source object"));
    }
    return new SourceSpatialMetadata(manifest, regions, ambient, nonAmbient, bodyNpc, touched);
  }

  identity(world: SpatialWorld, geometry: SpatialGeometry): string {
    return canonicalJson({ actor: world.player.id, tile: world.player.tile, scene: world.scene ?? null,
      base: world.ui?.appearance.base ?? null, dynamicObjects: world.dynamicObjects ?? null,
      authority: world.audioAuthority?.varps ?? null, geometry });
  }

  scene(world: SpatialWorld, geometry: SpatialGeometry): SourceAudioScene {
    if (!world.scene) missing("WorldView.scene is required to identify spatial metadata.", "scene_identity");
    validateScene(world.scene, world);
    if (!Object.hasOwn(geometry, "owner")) {
      missing("The host must supply its actual SourceWorldOwner or explicit root-view null; an instance ID is not an owner mapping.",
        "world_owner_required");
    }
    if (geometry.owner !== null) {
      if (!geometry.owner || typeof geometry.owner.id !== "string" || geometry.owner.id.length === 0) {
        missing("The source world-owner identity is invalid.", "world_owner_required");
      }
      integer(geometry.owner.exteriorPlane, 0, 3, "world-owner exterior plane");
      integer(geometry.owner.audibleInteriorPlane, 0, 3, "world-owner interior plane");
    }
    const view = geometry.placement;
    if (!view?.blocks || view.sizeTiles !== 104 || geometry.sceneId === null || geometry.loadedSquares === null
      || !Number.isSafeInteger(view.baseX) || !Number.isSafeInteger(view.baseY)
      || view.baseX % 8 !== 0 || view.baseY % 8 !== 0
      || geometry.loadedSquares.length === 0 || geometry.loadedSquares.length > 61
      || new Set(geometry.loadedSquares).size !== geometry.loadedSquares.length
      || geometry.loadedSquares.some(square => !Number.isSafeInteger(square) || square < 0 || square > 65535)
      || !inside(world.player.tile.x, world.player.tile.y, view)) {
      missing("The actual104x104 block scene containing the listener is not available.", "scene_geometry");
    }
    if (world.ui?.version !== 1 || world.ui.appearance.base?.sourceNpc !== this.#bodyNpc) {
      missing("The listener does not have the renderer's actual declared player-base binding.", "listener_binding");
    }
    const layout = rendererInstanceLayout(world, this.#manifest.instanceLayouts, world.scene.instanceTemplate);
    const sceneId = `blocks@${view.baseX},${view.baseY}${layout ? `#${layout.template}` : ""}`;
    if (geometry.sceneId !== sceneId) {
      missing("The active renderer scene does not identify its actual bounds and authoritative instance template.", "scene_identity");
    }
    const instance = world.scene.instance;
    const owner = geometry.owner === null ? null : Object.freeze({
      id: geometry.owner.id, exteriorPlane: geometry.owner.exteriorPlane,
      audibleInteriorPlane: geometry.owner.audibleInteriorPlane,
    });
    const rows: Placement[] = [];
    for (const square of geometry.loadedSquares) {
      const region = this.#regions.get(square);
      if (!region) missing(`Active square ${square} has no published original placement JSON in this ContentManifest.`, "region_coverage");
      if (!layout) {
        rows.push(...region.filter(row => inside(row.x, row.y, view)));
        continue;
      }
      for (const [index, chunk] of layout.chunks.entries()) {
        const sourceX = chunk.sourceChunkX * 8, sourceY = chunk.sourceChunkY * 8;
        for (const row of region) {
          if (row.plane !== chunk.sourcePlane || row.x < sourceX || row.x >= sourceX + 8 || row.y < sourceY || row.y >= sourceY + 8) continue;
          const placed = { ...row, id: `${row.id}:chunk:${index}`, plane: chunk.plane,
            x: row.x + chunk.chunkX * 8 - sourceX, y: row.y + chunk.chunkY * 8 - sourceY };
          if (inside(placed.x, placed.y, view)) rows.push(placed);
        }
      }
    }
    const emitters = new Map<string, SourceSceneEmitter>();
    const emit = (id: string, objectId: number, x: number, y: number, plane: number, orientation: number): void => {
      emitters.set(id, { id, objectId, tile: { x, y, plane }, orientation,
        instance, owner, present: true });
    };
    for (const row of rows) {
      if (this.#ambient.has(row.objectId)) emit(row.id, row.objectId, row.x, row.y, row.plane, row.orientation);
    }
    if (!Array.isArray(world.dynamicObjects)) missing("The current dynamic-object projection is unavailable.", "dynamic_objects_required");
    const dynamicIds = new Set<string>();
    for (const object of world.dynamicObjects) {
      if (object.instance !== world.scene.instance || !inside(object.tile.x, object.tile.y, view)) continue;
      if (!object.id || dynamicIds.has(object.id)) missing("Dynamic spatial identities are missing or duplicated.", "dynamic_identity");
      dynamicIds.add(object.id);
      const definition = object.objectId === undefined ? undefined : this.#manifest.catalog.entities[object.objectId];
      if (object.sourceId === undefined || !definition || definition.sourceId !== object.sourceId) {
        missing(`Dynamic object ${object.id} lacks validated canonical source metadata.`, "dynamic_identity");
      }
      integer(object.sourceId, 0, 0xffffffff, "dynamic source object ID");
      integer(object.tile.x, 0, 16383, "dynamic world x");
      integer(object.tile.y, 0, 16383, "dynamic world y");
      integer(object.tile.plane, 0, 3, "dynamic plane");
      const colocated = rows.filter(row => row.x === object.tile.x && row.y === object.tile.y && row.plane === object.tile.plane);
      const originals = colocated.filter(row => row.objectId === object.sourceId);
      if (originals.length === 0 && colocated.some(row => this.#ambient.has(row.objectId))) {
        missing(`Dynamic object ${object.id} needs an explicit source-placement association at its occupied audio tile.`, "dynamic_placement_identity");
      }
      if (this.#nonAmbient.has(object.sourceId)) continue;
      if (!sourceObjectDefinition(object.sourceId)) {
        missing(`Dynamic source object ${object.sourceId} has neither native calibration nor a verified no-stream definition.`, "dynamic_sound_metadata");
      }
      if (originals.length > 1) missing(`Dynamic object ${object.id} cannot select between multiple original placements.`, "dynamic_placement_identity");
      const original = originals[0];
      const orientation = object.quarterTurns ?? original?.orientation;
      if (orientation === undefined) missing(`Dynamic object ${object.id} has no actual quarterTurns or original placement orientation.`, "dynamic_orientation");
      integer(orientation, 0, 3, "dynamic quarter turns");
      if (original) emitters.delete(original.id);
      emit(object.id, object.sourceId, object.tile.x, object.tile.y, object.tile.plane, orientation);
    }
    // The published renderer ABI draws the current one-tile actor at its tile centre.
    const bounds = sourceObjectBounds(world.player.tile, 1, 1);
    const scene = sourceSceneAuthority(world, {
      listener: { x: (bounds.minX + bounds.maxX) / 2, y: (bounds.minY + bounds.maxY) / 2 },
      plane: world.player.tile.plane, instance, owner,
      emitters: [...emitters.values()], varps: new Map(),
    });
    return deepFreeze(scene);
  }
}
