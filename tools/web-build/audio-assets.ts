import { createHash } from "node:crypto";
import { lstat, mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { AUDIO_INPUTS } from "../../web/audio/index.ts";
import { SOURCE_PACK_SHA256 } from "../../web/shared/contracts.ts";
import type { AssetRecord } from "../../web/app/manifest.ts";
import type { PublicFile } from "./deliver.ts";

const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const hash = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

interface SourcePayload { id: string; path: string; sha256: string; bytes: number; url: string; contentType: string }

async function checked(path: string, bytes: number, digest: string): Promise<Buffer> {
  const input = resolve(root, path);
  if (!input.startsWith(root + sep) || await realpath(input) !== input) throw new Error("Audio inputs must be regular worktree files.");
  const stat = await lstat(input);
  if (!stat.isFile() || stat.size !== bytes || bytes > 64 * 1024 * 1024) throw new Error("Original audio size mismatch.");
  const data = await readFile(input);
  if (data.length !== bytes || hash(data) !== digest) throw new Error(`Original audio integrity mismatch: ${path}`);
  return data;
}

/** Byte delivery only. All selection, PCM, loop, gain and fade policy stays in the real audio factory. */
export async function deliverAudioAssets(directory: string): Promise<{
  assets: AssetRecord[]; files: PublicFile[]; aliases: Record<string, string>; metadata: string[]; bytes: number;
}> {
  const output = resolve(directory);
  if (!output.startsWith(root + sep)) throw new Error("Audio delivery must remain inside the worktree.");
  const payloads: SourcePayload[] = [];
  const documents: Record<string, unknown> = {};
  const metadataBytes = new Map<string, Buffer>();
  const metadata: string[] = [];
  for (const [name, input] of Object.entries(AUDIO_INPUTS)) {
    const data = await checked(input.path, input.bytes, input.sha256);
    metadataBytes.set(input.path, data);
    documents[name] = JSON.parse(data.toString("utf8"));
    const id = `asset.source.audio.metadata.${name}`;
    metadata.push(id);
    payloads.push({ id, path: input.path, sha256: input.sha256, bytes: input.bytes,
      url: `/content/audio/${name}.json`, contentType: "application/json" });
  }
  const manifest = documents.manifest as { assets: Array<{
    asset_id: string; path: string; sha256: string; size_bytes: number; kind: string; source_group: number;
  }> };
  const reference = documents.reference as { reference_templates: Array<{
    id: string; path: string; sha256: string; size_bytes: number; source_group: number;
  }> };
  const supplement = documents.supplement as {
    source_pack_sha256: string; base_manifest: { path: string; sha256: string };
    frozen_files_modified: boolean; reencoded_base_files: number;
    assets: Array<{ asset_id: string; path: string; sha256: string; size_bytes: number;
      kind: "music" | "jingle"; source_group: number; native_mixer_level: number }>;
  };
  if (manifest.assets.length !== 264 || reference.reference_templates.length !== 2) throw new Error("Actual audio factory input cardinality changed.");
  if (supplement.source_pack_sha256 !== SOURCE_PACK_SHA256
    || supplement.base_manifest.path !== AUDIO_INPUTS.manifest.path
    || supplement.base_manifest.sha256 !== AUDIO_INPUTS.manifest.sha256
    || supplement.frozen_files_modified !== false || supplement.reencoded_base_files !== 0
    || supplement.assets.length !== 9) throw new Error("The additive audio publication must retain the exact frozen base.");
  for (const asset of manifest.assets) {
    payloads.push({ id: asset.asset_id, path: asset.path, sha256: asset.sha256, bytes: asset.size_bytes,
      url: `/assets/audio/${asset.kind}-${asset.source_group}.flac`, contentType: "audio/flac" });
  }
  for (const asset of reference.reference_templates) {
    payloads.push({ id: asset.id, path: asset.path, sha256: asset.sha256, bytes: asset.size_bytes,
      url: `/assets/audio/reference-${asset.source_group}.wav`, contentType: "audio/wav" });
  }
  for (const asset of supplement.assets) {
    if (!["music", "jingle"].includes(asset.kind) || !Number.isSafeInteger(asset.source_group)
      || asset.asset_id !== `asset.source.osrs.cache2695.audio-supplement.${asset.kind}.${asset.source_group}.native255`
      || asset.path !== `assets/source/osrs/audio-supplement/${asset.kind}/${asset.source_group}-native255.flac`
      || asset.native_mixer_level !== 255) throw new Error("Invalid native supplement delivery identity.");
    payloads.push({ id: asset.asset_id, path: asset.path, sha256: asset.sha256, bytes: asset.size_bytes,
      url: `/${asset.path}`, contentType: "audio/flac" });
  }
  if (new Set(payloads.map((asset) => asset.id)).size !== payloads.length
    || new Set(payloads.map((asset) => asset.url)).size !== payloads.length) throw new Error("Duplicate audio delivery identity.");
  const assets: AssetRecord[] = [];
  const files: PublicFile[] = [];
  const aliases: Record<string, string> = {};
  let bytes = 0;
  for (const asset of payloads) {
    const data = metadataBytes.get(asset.path) ?? await checked(asset.path, asset.bytes, asset.sha256);
    const target = resolve(output, asset.url.slice(1));
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, data, { flag: "wx" });
    assets.push({ id: asset.id, url: asset.url, sha256: asset.sha256, bytes: asset.bytes, contentType: asset.contentType });
    files.push({ url: asset.url, path: asset.url.slice(1), sha256: asset.sha256, content_type: asset.contentType });
    aliases[asset.path] = asset.id;
    bytes += data.length;
  }
  return { assets, files, aliases, metadata, bytes };
}
