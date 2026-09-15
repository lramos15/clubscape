import { execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { SOURCE_PACK_SHA256 } from "../../web/shared/contracts.ts";
import { parseContentManifest } from "../../web/app/manifest.ts";
import type { AssetRecord } from "../../web/app/manifest.ts";
import { canonicalJson } from "../../web/app/identity.ts";
import { projectArtifact, readArtifact } from "./artifact.ts";
import type { PublicFile } from "./deliver.ts";
import { deliverAudioAssets } from "./audio-assets.ts";
import { captureSourceRunPin } from "./run-pins.ts";
import { deliverUiAssets } from "./ui-assets.ts";
import { deliverRenderAssets } from "./render-assets.ts";

const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const sha = (bytes: Uint8Array | string): string => createHash("sha256").update(bytes).digest("hex");

/** Explicit canonical source/component delivery; accounts and characters are never seeded. */
export async function prepareSourceBundle(directory: string, worldId: string): Promise<{
  directory: string; artifactSha256: string; contentRevision: string; publicAssets: number; publicBytes: number;
  worldId: string;
  serverDescriptorBytes: number; serverDescriptorLimit: number; serverCompatible: boolean;
}> {
  const output = resolve(root, directory);
  if (!output.startsWith(root + sep)) throw new Error("Source delivery must stay inside this worktree.");
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(worldId)
    || worldId === "00000000-0000-0000-0000-000000000000") throw new Error("Provide a real non-nil world UUID.");
  const manifest = JSON.parse(await readFile(resolve(root, "content/m1/manifest.json"), "utf8")) as {
    revision: string; compiled_artifact: { path: string; sha256: string; uncompressed_sha256: string };
  };
  const compressed = await readFile(resolve(root, manifest.compiled_artifact.path));
  if (sha(compressed) !== manifest.compiled_artifact.sha256) throw new Error("Canonical compressed artifact changed.");
  const bytes = await readArtifact(manifest.compiled_artifact.path);
  const projection = await projectArtifact(manifest.compiled_artifact.path);
  if (sha(bytes) !== manifest.compiled_artifact.uncompressed_sha256
    || projection.artifactSha256 !== sha(bytes) || projection.catalog.contentRevision !== manifest.revision) {
    throw new Error("Canonical artifact/projection identity mismatch.");
  }
  await mkdir(dirname(output), { recursive: true });
  await mkdir(output, { mode: 0o700 });
  const source = JSON.parse(execFileSync("python3", ["tools/web-build/source-records.py"], {
    cwd: root, encoding: "utf8",
    input: JSON.stringify({ assetIds: projection.referencedAssets, directory: output }),
    maxBuffer: 8 * 1024 * 1024, timeout: 180_000,
  })) as { assets: AssetRecord[]; files: PublicFile[]; inventoryActions: Record<string, string[]>; bytes: number };
  const audio = await deliverAudioAssets(output);
  const ui = await deliverUiAssets(output);
  const render = await deliverRenderAssets(output);
  source.assets.push(...audio.assets);
  source.assets.push(...ui.assets);
  source.assets.push(...render.assets);
  source.files.push(...audio.files);
  source.files.push(...ui.files);
  source.files.push(...render.files);
  source.bytes += audio.bytes;
  source.bytes += ui.bytes;
  source.bytes += render.bytes;
  const actions: Record<string, string[]> = {};
  for (const [id, definition] of Object.entries(projection.catalog.items)) {
    if (definition.asset !== null && source.inventoryActions[definition.asset] !== undefined) {
      actions[id] = source.inventoryActions[definition.asset]!;
    }
  }
  const content = parseContentManifest({
    schemaVersion: 1, sourcePackSha256: SOURCE_PACK_SHA256, contentRevision: manifest.revision,
    artifactSha256: sha(bytes), catalog: { ...projection.catalog, inventoryActions: actions },
    contentValidation: projection.contentValidation, assets: source.assets, aliases: { ...audio.aliases, ...ui.aliases },
    instanceLayouts: projection.instanceLayouts,
    bootstrap: [...audio.metadata, ...ui.startup], rendererManifest: "asset.source.render.manifest", renderer: render.renderer,
    regions: Object.fromEntries(Object.entries(projection.regions).map(([id, region]) => {
      if (region.sceneAsset === null) throw new Error(`Canonical region ${id} has no source scene asset.`);
      const block = render.renderer.coverage === "source_world_blocks" ? render.renderer.regions[id] : undefined;
      return [id, { sceneId: block ? id : region.sceneAsset, sceneAsset: region.sceneAsset,
        requiredAssets: [region.sceneAsset, ...(block?.requiredAssets ?? [])],
        routeId: id, workloadId: "unconfigured", camera: null, controls: null }];
    })),
  });
  const contentBytes = canonicalJson(content) + "\n";
  await mkdir(resolve(output, "content"), { recursive: true });
  await writeFile(resolve(output, "content/manifest.json"), contentBytes);
  await writeFile(resolve(output, "world.csc"), bytes);
  source.files.push({
    url: "/content/manifest.json", path: "content/manifest.json", sha256: sha(contentBytes), content_type: "application/json",
  });
  const publicManifest = JSON.stringify({ schema_version: 1, files: source.files }) + "\n";
  if (Buffer.byteLength(publicManifest) > 2 * 1024 * 1024) throw new Error("Public source delivery manifest exceeds its byte budget.");
  await writeFile(resolve(output, "clubscape-game-assets.json"), publicManifest);
  const urls = new Map(source.assets.map((asset) => [asset.id, asset.url]));
  const gameDescriptor = JSON.stringify({
    schema_version: 1, world_id: worldId, artifact: "world.csc", sha256: sha(bytes),
    content_manifest_path: "/content/manifest.json",
    assets: Object.fromEntries(projection.referencedAssets.map((id) => {
      const url = urls.get(id);
      if (!url) throw new Error(`Compiled source asset ${id} is missing.`);
      return [id, url];
    })),
    // Selection only: the server computes the proofs and validates every restored/mutated world.
    readiness_profile: { id: "ordinary_normal_f2p", excluded_items: ["item.ensouled_goblin_head", "item.milk.bottomless_bucket"] },
  }) + "\n";
  await writeFile(resolve(output, "clubscape-game.json"), gameDescriptor, { mode: 0o600 });
  await writeFile(resolve(output, "source-run-pin.json"), JSON.stringify(await captureSourceRunPin(output), null, 2) + "\n",
    { flag: "wx", mode: 0o600 });
  return { directory: output, worldId, artifactSha256: sha(bytes), contentRevision: manifest.revision,
    publicAssets: source.assets.length, publicBytes: source.bytes + Buffer.byteLength(contentBytes),
    serverDescriptorBytes: Buffer.byteLength(gameDescriptor), serverDescriptorLimit: 512 * 1024,
    serverCompatible: Buffer.byteLength(gameDescriptor) <= 512 * 1024 };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const output = process.argv[2];
  if (!output) throw new Error("Usage: pnpm source:bundle <new worktree-relative output directory> [stable world UUID]");
  const result = await prepareSourceBundle(output, process.argv[3] ?? randomUUID());
  console.log(JSON.stringify({ ...result, kind: "real-source-definition-delivery", seededCharacters: false, presentationComplete: false }));
  if (!result.serverCompatible) {
    console.error("Actual canonical asset membership exceeds the server's game-descriptor byte limit. Static source delivery is valid; game startup is blocked. No asset IDs or source data were removed.");
    process.exitCode = 2;
  }
}
