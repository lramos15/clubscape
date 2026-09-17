import type { RenderAssetManifest, RendererDiagnostics, ScenePlacement } from "../renderer/src/index.ts";
import { invariant } from "./errors.ts";

export function sourceScenePlacement(state: RendererDiagnostics, raw: ScenePlacement | null, instanceTemplate: string | null = null): ScenePlacement | null {
  if (state.sceneBase === null) return raw;
  const { x, y } = state.sceneBase;
  invariant(raw !== null && raw.baseX === x && raw.baseY === y && raw.sizeTiles === 104
    && state.sceneId === `blocks@${x},${y}${instanceTemplate === null ? "" : `#${instanceTemplate}`}` && state.loadedSquares.length > 0,
  "Native scene placement disagrees with the completed streamed-scene assembly.", "renderer");
  // The relayed WASM tests scene_id().is_none() for this flag, although assembled scenes
  // have a blocks@ ID. Use the public adapter's actual completed-assembly diagnostics.
  return { ...raw, blocks: true };
}

export function residentRendererAssets(manifest: RenderAssetManifest, state: RendererDiagnostics): RendererDiagnostics["assets"] {
  const blocks = new Map((manifest.blocks ?? []).flatMap((block) =>
    [block.file, block.file_gz, block.models_file, block.models_file_gz]
      .filter((path): path is string => path !== undefined).map((path) => [path, block.square] as const)));
  for (const block of manifest.minimap_blocks ?? []) blocks.set(block.file, block.square);
  const scenes = new Map(manifest.scenes.flatMap((scene) =>
    [scene.file, scene.file_gz, scene.models_file, scene.models_file_gz]
      .filter((path): path is string => path !== undefined).map((path) => [path, scene.name] as const)));
  const resident = new Set(state.loadedSquares);
  const assets = new Map<string, RendererDiagnostics["assets"][number]>();
  for (const asset of state.assets) {
    const block = blocks.get(asset.id), scene = scenes.get(asset.id);
    const loaded = asset.loaded && (block === undefined || resident.has(block))
      && (scene === undefined || state.sceneId === scene);
    assets.set(asset.id, { ...asset, loaded });
  }
  return [...assets.values()];
}
