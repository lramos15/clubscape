import { createHash } from "node:crypto";
import { lstat, mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { SOURCE_PACK_SHA256 } from "../../web/shared/contracts.ts";
import type { UiCatalogue } from "../../web/ui/assets.ts";
import { decodeUiCatalogue } from "../../web/ui/assets.ts";
import type { AssetRecord } from "../../web/app/manifest.ts";
import type { PublicFile } from "./deliver.ts";

const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const hash = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");
interface Pin { path: string; sha256: string; bytes: number }

export function uiRuntimeImages(catalogue: UiCatalogue, pins: ReadonlyMap<string, Pin>): string[] {
  const minimap = [...pins.keys()].filter((path) => /^ui\/minimaps\/(?:compass|dot-[0-9]+)\.png$/.test(path));
  for (const path of ["ui/minimaps/compass.png", "ui/minimaps/dot-1.png", "ui/minimaps/dot-2.png"]) {
    if (!minimap.includes(path)) throw new Error(`The actual UI requires an unpublished source minimap primitive: ${path}`);
  }
  return [...new Set([
    catalogue.titleBackground,
    ...Object.values(catalogue.sprites).map((sprite) => sprite.asset),
    ...Object.values(catalogue.portraits).map((portrait) => portrait.asset),
    ...Object.values(catalogue.staticModels).map((model) => model.asset),
    ...catalogue.minimaps.map((map) => map.asset), ...minimap,
    ...Object.values(catalogue.items).flatMap((item) => item.icons.flatMap((icon) =>
      [icon.asset, icon.selectedAsset, icon.zeroShadowAsset])),
  ])].sort();
}

/** Deliver catalogue-selected original UI primitives, never reference panels/captures. */
export async function deliverUiAssets(directory: string): Promise<{
  assets: AssetRecord[]; files: PublicFile[]; aliases: Record<string, string>; bytes: number;
  startup: string[];
}> {
  const output = resolve(directory);
  if (!output.startsWith(root + sep)) throw new Error("UI delivery must remain in this worktree.");
  const provenanceBytes = await readFile(resolve(root, "assets/compiled/ui/provenance.json"));
  const provenance = JSON.parse(provenanceBytes.toString("utf8")) as { sourcePackSha256: string; assets: Pin[] };
  if (provenance.sourcePackSha256 !== SOURCE_PACK_SHA256) throw new Error("UI provenance belongs to another source pack.");
  const pins = new Map(provenance.assets.map((record) => [record.path, record]));
  if (pins.size !== provenance.assets.length) throw new Error("Duplicate UI provenance paths.");
  const checked = async (id: string): Promise<Buffer> => {
    const pin = pins.get(id);
    if (!pin || !/^ui\/[A-Za-z0-9_./-]+$/.test(id) || id.split("/").some((part) => !part || part.startsWith("."))) {
      throw new Error("A requested UI asset has no valid original provenance.");
    }
    const path = resolve(root, "assets/compiled", id);
    if (await realpath(path) !== path || !path.startsWith(resolve(root, "assets/compiled/ui") + sep)) throw new Error("UI input escaped its compiled asset root.");
    const stat = await lstat(path);
    if (!stat.isFile() || stat.size !== pin.bytes || pin.bytes > 64 * 1024 * 1024) throw new Error("UI source asset size mismatch.");
    const data = await readFile(path);
    if (data.length !== pin.bytes || hash(data) !== pin.sha256) throw new Error(`UI source asset hash mismatch: ${id}`);
    return data;
  };
  const catalogueBytes = await checked("ui/manifest.json");
  const catalogue = decodeUiCatalogue(JSON.parse(catalogueBytes.toString("utf8")));
  if (catalogue.version !== 1 || catalogue.sourcePackSha256 !== SOURCE_PACK_SHA256 || catalogue.sourceCache !== 2695) {
    throw new Error("UI catalogue is not the approved source-cache projection.");
  }
  const images = uiRuntimeImages(catalogue, pins);
  const assets: AssetRecord[] = [], files: PublicFile[] = [];
  const aliases: Record<string, string> = {};
  let bytes = 0;
  const emit = async (sourceId: string, id: string, route: string, data: Buffer, mime: string): Promise<void> => {
    const target = resolve(output, route.slice(1));
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, data, { flag: "wx" });
    const sha256 = hash(data);
    assets.push({ id, url: route, sha256, bytes: data.length, contentType: mime });
    files.push({ url: route, path: route.slice(1), sha256, content_type: mime });
    aliases[sourceId] = id;
    bytes += data.length;
  };
  await emit("ui/manifest.json", "asset.source.ui.catalogue", "/content/ui/manifest.json", catalogueBytes, "application/json");
  await emit("ui/provenance.json", "asset.source.ui.provenance", "/content/ui/provenance.json", provenanceBytes, "application/json");
  let index = 0;
  for (const id of images) {
    if (id !== "ui/title-background.png" && !/^ui\/(?:sprites|items|portraits|minimaps)\/[A-Za-z0-9_-]+\.png$/.test(id)
      && !/^ui\/models\/[0-9a-f]{64}\.png$/.test(id)) {
      throw new Error(`Non-primitive UI image reference is not eligible for client delivery: ${id}`);
    }
    await emit(id, `asset.source.ui.image.${index++}`, `/assets/${id}`, await checked(id), "image/png");
  }
  const startup = ["asset.source.ui.catalogue", "asset.source.ui.provenance",
    ...[catalogue.titleBackground, ...Object.values(catalogue.sprites).map((sprite) => sprite.asset)]
      .map((id) => aliases[id]!)];
  return { assets, files, aliases, bytes, startup: [...new Set(startup)] };
}
