import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { SOURCE_PACK_SHA256 } from "../../web/shared/contracts.ts";
import { parseContentManifest } from "../../web/app/manifest.ts";
import type { ContentManifest, DisplayCatalog, RegionPresentation } from "../../web/app/manifest.ts";
import { publicAssetPath, publicFile } from "./deliver.ts";
import { projectArtifact } from "./artifact.ts";

const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const options = new Map<string, string>();
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index];
  const value = process.argv[index + 1];
  if (!key || !["--artifact", "--bindings", "--asset-root", "--output"].includes(key) || !value || options.has(key)) {
    throw new Error("Usage: pnpm content --artifact <world.csc> --bindings <compiled presentation.json> --asset-root <public root> --output <content/manifest.json>");
  }
  options.set(key, value);
}
function path(key: string): string {
  const value = options.get(key);
  if (!value) throw new Error(`Missing ${key}. No source/presentation defaults are fabricated.`);
  const path = resolve(root, value);
  if (!path.startsWith(root + sep)) throw new Error("Content build paths must stay inside this worktree.");
  return path;
}
const projection = await projectArtifact(path("--artifact"));
const bindings = JSON.parse(await readFile(path("--bindings"), "utf8")) as Pick<ContentManifest,
  "sourcePackSha256" | "assets" | "bootstrap" | "rendererManifest" | "aliases" | "renderer"> & {
    regions: Record<string, RegionPresentation>; icons?: Record<string, string>;
    inventoryActions?: Record<string, string[]>;
    catalog?: Pick<DisplayCatalog, "icons" | "inventoryActions">;
  };
if (bindings.sourcePackSha256 !== SOURCE_PACK_SHA256) throw new Error("Presentation bindings do not match the approved source pack.");
for (const [id, region] of Object.entries(bindings.regions)) {
  if (projection.regions[id]?.sceneAsset !== region.sceneAsset) {
    throw new Error(`Presentation region ${id} does not retain its compiler-validated scene asset ID.`);
  }
}
const sourceAssets = new Set(bindings.assets.map((asset) => asset.id));
for (const id of projection.referencedAssets) {
  if (!sourceAssets.has(id)) throw new Error(`Required compiled content asset has no public delivery binding: ${id}.`);
}
const manifest = parseContentManifest({
  schemaVersion: 1, sourcePackSha256: SOURCE_PACK_SHA256, contentRevision: projection.catalog.contentRevision,
  artifactSha256: projection.artifactSha256,
  catalog: { ...projection.catalog, icons: bindings.icons ?? bindings.catalog?.icons ?? projection.catalog.icons ?? {},
    inventoryActions: bindings.inventoryActions ?? bindings.catalog?.inventoryActions ?? {} },
  contentValidation: projection.contentValidation,
  ...(bindings.aliases === undefined ? {} : { aliases: bindings.aliases }),
  ...(bindings.renderer === undefined ? {} : { renderer: bindings.renderer }),
  assets: bindings.assets, bootstrap: bindings.bootstrap, rendererManifest: bindings.rendererManifest, regions: bindings.regions,
});
for (const asset of manifest.assets) await publicFile(path("--asset-root"), publicAssetPath(asset.url), asset.url, asset);
const output = path("--output");
if ([path("--artifact"), path("--bindings")].includes(output)) throw new Error("Output cannot overwrite a source input.");
await mkdir(dirname(output), { recursive: true });
await writeFile(output, JSON.stringify(manifest, null, 2) + "\n");
console.log(JSON.stringify({
  kind: "public-source-content-projection", assets: manifest.assets.length,
  regions: Object.keys(manifest.regions).length, artifactSha256: projection.artifactSha256,
  sourcePackSha256: SOURCE_PACK_SHA256, gameplayAccepted: false, presentationAccepted: false,
}));
