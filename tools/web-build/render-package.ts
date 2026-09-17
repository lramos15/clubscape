import { createHash } from "node:crypto";
import { isDeepStrictEqual } from "node:util";
import type { RenderAssetManifest } from "../../web/renderer/src/index.ts";
import { isHash, publicPath } from "../../web/app/identity.ts";

export interface RenderBlockPackageFile {
  file: string;
  sha256: string;
  size_bytes: number;
  decompressed?: string;
  decompressed_sha256?: string;
}
export interface RenderBlockPackage {
  manifest_sha256: string;
  content_sha256: string;
  pack: { file: string; sha256: string; size_bytes: number };
  files: RenderBlockPackageFile[];
  squares: number[];
}

function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid published block-package record.");
  return value as Record<string, unknown>;
}

/** Validate the whole published index before the current exporter writes any package member. */
export function validateRenderBlockPackage(value: unknown, manifest: RenderAssetManifest, manifestSha256: string): RenderBlockPackage {
  const index = object(value), pack = object(index.pack);
  if (index.schema_version !== 1 || index.kind !== "clubscape_render_blocks"
    || index.manifest_sha256 !== manifestSha256 || !isHash(manifestSha256)
    || index.approved_reference_pack_sha256 !== manifest.approved_reference_pack_sha256
    || !Array.isArray(index.files) || !isHash(index.content_sha256)
    || typeof pack.file !== "string" || !/^clubscape-render-blocks-[0-9a-f]{16}\.tar$/.test(pack.file)
    || !isHash(pack.sha256) || typeof pack.size_bytes !== "number" || !Number.isSafeInteger(pack.size_bytes)
    || pack.size_bytes < 1 || pack.size_bytes > 256 * 1024 * 1024) {
    throw new Error("Block package must retain the exact current manifest/source/pack binding.");
  }
  const expected = [
    ...(manifest.blocks ?? []).flatMap((block) => {
      if (!block.file_gz || !block.models_file_gz) throw new Error("Published block package requires both original gzip twins.");
      return [block.file_gz, block.models_file_gz];
    }),
    ...(manifest.minimap_blocks ?? []).map((block) => block.file),
  ];
  if (new Set(expected).size !== expected.length || index.files.length !== expected.length
    || !isDeepStrictEqual(index.squares, (manifest.blocks ?? []).map((block) => block.square))) {
    throw new Error("Block package does not contain the exact source squares, twins and minimap sidecars.");
  }
  const files: RenderBlockPackageFile[] = index.files.map((value, position) => {
    const entry = object(value), name = expected[position]!;
    publicPath(`/assets/compiled/render/${name}`);
    const pin = manifest.files[name];
    if (!pin || entry.file !== name || entry.sha256 !== pin.sha256 || entry.size_bytes !== pin.size_bytes) {
      throw new Error(`Block package member differs from its current published pin: ${name}`);
    }
    const record: RenderBlockPackageFile = { file: name, sha256: pin.sha256, size_bytes: pin.size_bytes };
    if (pin.detail?.encoding === "gzip") {
      if (typeof pin.detail.decompressed !== "string" || !isHash(pin.detail.decompressed_sha256)) {
        throw new Error(`Published gzip has no exact raw identity: ${name}`);
      }
      record.decompressed = pin.detail.decompressed;
      record.decompressed_sha256 = pin.detail.decompressed_sha256;
    }
    if (!isDeepStrictEqual(entry, record)) throw new Error(`Block package changed raw/gzip identity: ${name}`);
    if (record.decompressed !== undefined) {
      publicPath(`/assets/compiled/render/${record.decompressed}`);
      if (manifest.files[record.decompressed]?.sha256 !== record.decompressed_sha256) {
        throw new Error(`Block package raw twin has no matching manifest pin: ${name}`);
      }
    }
    return record;
  });
  const content = createHash("sha256").update(files.map((entry) => `${entry.file} ${entry.sha256}`).join("\n")).digest("hex");
  if (content !== index.content_sha256 || pack.file !== `clubscape-render-blocks-${content.slice(0, 16)}.tar`) {
    throw new Error("Block package content identity does not match the complete member list.");
  }
  return { manifest_sha256: manifestSha256, content_sha256: content, files,
    squares: (manifest.blocks ?? []).map((block) => block.square),
    pack: { file: pack.file, sha256: pack.sha256, size_bytes: pack.size_bytes } };
}
