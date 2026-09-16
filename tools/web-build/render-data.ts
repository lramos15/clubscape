import { isDeepStrictEqual } from "node:util";
import type { RenderAssetManifest } from "../../web/renderer/src/index.ts";
import { publicPath } from "../../web/app/identity.ts";

export const DEFAULT_RENDER_INPUTS = ".local/render-inputs-78fed3ca";

export function renderRuntimeFiles(manifest: RenderAssetManifest): {
  common: string[]; scenes: Map<string, string[]>; blocks: Map<number, string[]>; files: string[];
} {
  const floors = manifest.floor_definitions;
  if ((manifest.blocks?.length ?? 0) > 0 && !floors) {
    throw new Error("Runtime renderer block scenes require source floor definitions.");
  }
  if (floors && floors.sha256 !== manifest.files[floors.file]?.sha256) {
    throw new Error("Runtime renderer floor definitions have no matching published pin.");
  }
  const common = new Set([
    "palette.bin", ...(floors ? [floors.file] : []), ...manifest.textures.map((id) => `textures/${id}.bin`),
    ...(manifest.files["minimap/mapscenes.bin"] ? ["minimap/mapscenes.bin"] : []),
    ...(manifest.files["minimap/mapicons.bin"] ? ["minimap/mapicons.bin"] : []),
    ...(manifest.gear_pose_fits ? [manifest.gear_pose_fits.file] : []),
    ...manifest.npcs.map((npc) => npc.pack),
    ...(manifest.sequences ?? []).map((sequence) => sequence.file),
    ...(manifest.npc_definitions ?? []).map((npc) => npc.base_model),
    ...(manifest.player_reference ? [manifest.player_reference.model] : []),
    ...(manifest.equipment_items ?? []).flatMap((item) => item.equip_model ? [item.equip_model] : []),
    ...(manifest.dynamic_objects ?? []).flatMap((object) =>
      object.variants.flatMap((variant) => [variant.model, ...(variant.frames ?? [])])),
    ...(manifest.ground_items ?? []).flatMap((item) => item.variants.map((variant) => variant.model)),
  ]);
  const scenes = new Map(manifest.scenes.map((scene) =>
    [scene.name, [scene.file_gz ?? scene.file, scene.models_file_gz ?? scene.models_file]]));
  const minimaps = new Map((manifest.minimap_blocks ?? []).map((block) => [block.square, block.file]));
  const blocks = new Map((manifest.blocks ?? []).map((block) => {
    const minimap = minimaps.get(block.square);
    const paths = [block.file_gz ?? block.file, block.models_file_gz ?? block.models_file, ...(minimap ? [minimap] : [])];
    return [block.square, paths] as const;
  }));
  if (scenes.size !== manifest.scenes.length || blocks.size !== (manifest.blocks?.length ?? 0)) {
    throw new Error("Duplicate source renderer scene/block identity.");
  }
  const files = [...new Set([...common, ...[...scenes.values()].flat(), ...[...blocks.values()].flat()])];
  for (const name of files) {
    publicPath(`/assets/compiled/render/${name}`);
    if (!manifest.files[name]) throw new Error(`Runtime renderer input has no published pin: ${name}`);
  }
  return { common: [...common], scenes, blocks, files };
}

/** The exporter inventories files on disk; absent validation/raw twins do not change runtime inputs. */
export function verifyReproductionManifest(published: RenderAssetManifest, reproduced: RenderAssetManifest): string[] {
  const { files: expected, ...sourceMetadata } = published;
  const { files: actual, ...reproducedMetadata } = reproduced;
  if (!isDeepStrictEqual(sourceMetadata, reproducedMetadata)) throw new Error("Reproduction changed source renderer metadata.");
  for (const [name, pin] of Object.entries(actual)) {
    if (!isDeepStrictEqual(pin, expected[name])) throw new Error(`Reproduction changed a published input: ${name}`);
  }
  const omitted = Object.keys(expected).filter((name) => !Object.hasOwn(actual, name));
  for (const name of omitted) {
    if (name !== "tables.bin" && !name.startsWith("models/baked/")
      && !((name.startsWith("scenes/") || name.startsWith("blocks/")) && !name.endsWith(".gz")
        && Object.hasOwn(actual, `${name}.gz`))) {
      throw new Error(`Reproduction omitted a required published input: ${name}`);
    }
  }
  for (const name of renderRuntimeFiles(published).files) {
    if (!Object.hasOwn(actual, name)) throw new Error(`Reproduction omitted a runtime renderer input: ${name}`);
  }
  return omitted;
}
