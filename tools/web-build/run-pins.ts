import { createHash } from "node:crypto";
import { lstat, readFile, realpath } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { canonicalJson, publicPath } from "../../web/app/identity.ts";
import { parseContentManifest } from "../../web/app/manifest.ts";

const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const digest = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

export interface SourceRunPin {
  schemaVersion: 1;
  worldId: string;
  artifactSha256: string;
  contentRevision: string;
  sourcePackSha256: string;
  descriptorSha256: string;
  gameAssetsManifestSha256: string;
  contentManifestSha256: string;
}

async function bounded(directory: string, relative: string, maximum: number): Promise<Buffer> {
  if (!/^[A-Za-z0-9_][A-Za-z0-9_./-]*$/.test(relative)
    || relative.split("/").some((part) => !part || part.startsWith("."))) throw new Error("Unsafe pinned bundle path.");
  const path = resolve(directory, relative);
  if (!path.startsWith(directory + sep) || await realpath(path) !== path) throw new Error("Pinned files cannot escape or traverse symlinks.");
  const stat = await lstat(path);
  if (!stat.isFile() || stat.size > maximum) throw new Error("Pinned bundle file exceeds its byte budget.");
  const bytes = await readFile(path);
  if (bytes.length !== stat.size) throw new Error("Pinned bundle file changed during reading.");
  return bytes;
}

/** Capture an existing run's own identity; deliberately never consult the latest canonical source. */
export async function captureSourceRunPin(directory: string): Promise<SourceRunPin> {
  const folder = resolve(root, directory);
  if (!folder.startsWith(root + sep) || await realpath(folder) !== folder) throw new Error("Pinned runs must stay inside the worktree.");
  // Reading a pin is not admission by the server's smaller descriptor limit.
  const descriptorBytes = await bounded(folder, "clubscape-game.json", 2 * 1024 * 1024);
  const descriptor = JSON.parse(descriptorBytes.toString("utf8")) as {
    schema_version: number; world_id: string; artifact: string; sha256: string; content_manifest_path: string;
  };
  if (descriptor.schema_version !== 1 || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(descriptor.world_id)
    || descriptor.world_id === "00000000-0000-0000-0000-000000000000") throw new Error("Invalid pinned world identity.");
  publicPath(descriptor.content_manifest_path, "/content/");
  const artifact = await bounded(folder, descriptor.artifact, 64 * 1024 * 1024);
  const assetsBytes = await bounded(folder, "clubscape-game-assets.json", 2 * 1024 * 1024);
  const assets = JSON.parse(assetsBytes.toString("utf8")) as {
    schema_version: number; files: Array<{ url: string; path: string; sha256: string }>;
  };
  if (assets.schema_version !== 1 || !Array.isArray(assets.files)) throw new Error("Invalid pinned game asset manifest.");
  const records = assets.files.filter((record) => record.url === descriptor.content_manifest_path);
  if (records.length !== 1) throw new Error("Pinned public content route is missing or duplicated.");
  const contentBytes = await bounded(folder, records[0]!.path, 8 * 1024 * 1024);
  const content = parseContentManifest(JSON.parse(contentBytes.toString("utf8")));
  const artifactSha256 = digest(artifact);
  if (descriptor.sha256 !== artifactSha256 || content.artifactSha256 !== artifactSha256
    || records[0]!.sha256 !== digest(contentBytes)) throw new Error("Pinned artifact, descriptor and public content identities do not match.");
  return {
    schemaVersion: 1, worldId: descriptor.world_id, artifactSha256, contentRevision: content.contentRevision,
    sourcePackSha256: content.sourcePackSha256, descriptorSha256: digest(descriptorBytes),
    gameAssetsManifestSha256: digest(assetsBytes), contentManifestSha256: digest(contentBytes),
  };
}

export async function assertSourceRunPin(directory: string, expected: SourceRunPin): Promise<void> {
  if (canonicalJson(await captureSourceRunPin(directory)) !== canonicalJson(expected)) {
    throw new Error("Pinned run identity changed. Retain the original run or create a separate fresh isolated candidate; never reseed or silently replace it.");
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const directory = process.argv[2];
  if (!directory) throw new Error("Pass the existing source bundle directory to capture its immutable run pin.");
  console.log(JSON.stringify(await captureSourceRunPin(directory)));
}
