import { createHash } from "node:crypto";
import { lstat, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { SOURCE_PACK_SHA256 } from "../../web/shared/contracts.ts";
import type { RenderAssetManifest } from "../../web/renderer/src/index.ts";
import type { AssetRecord, RendererDelivery } from "../../web/app/manifest.ts";
import type { PublicFile } from "./deliver.ts";
import { publicPath } from "../../web/app/identity.ts";

export const RENDER_MANIFEST_SHA256 = "3fd1ec1953183de5537a2e7d239389c8dceed50c2e5d658dcc82c49113468845";
const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));
const hash = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

export async function deliverRenderAssets(directory: string): Promise<{
  assets: AssetRecord[]; files: PublicFile[]; renderer: RendererDelivery; bytes: number;
}> {
  const source = resolve(root, "assets/compiled/render");
  const data = await readFile(resolve(source, "manifest.json"));
  if (hash(data) !== RENDER_MANIFEST_SHA256) throw new Error("The relayed original renderer manifest identity changed.");
  const manifest = JSON.parse(data.toString("utf8")) as RenderAssetManifest;
  if (manifest.approved_reference_pack_sha256 !== SOURCE_PACK_SHA256 || manifest.kind !== "clubscape_render_assets") {
    throw new Error("Renderer source-pack identity mismatch.");
  }
  const assets: AssetRecord[] = [], files: PublicFile[] = [];
  const assetIds: Record<string, string> = {};
  let total = 0, index = 0;
  const emit = async (id: string, url: string, path: string, bytes: Buffer, type: string) => {
    publicPath(url);
    const target = resolve(directory, path);
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, bytes, { flag: "wx" });
    const sha256 = hash(bytes);
    assets.push({ id, url, sha256, bytes: bytes.length, contentType: type });
    files.push({ url, path, sha256, content_type: type });
    total += bytes.length;
  };
  await emit("asset.source.render.manifest", "/assets/compiled/render/manifest.json",
    "assets/compiled/render/manifest.json", data, "application/json");
  for (const [name, expected] of Object.entries(manifest.files)) {
    publicPath(`/assets/compiled/render/${name}`);
    const input = resolve(source, name);
    const stat = await lstat(input).catch(() => null);
    if (stat === null && (name === "tables.bin" || name.startsWith("models/baked/"))) continue;
    if (!stat?.isFile() || stat.isSymbolicLink() || stat.size !== expected.size_bytes || stat.size > 64 * 1024 * 1024) {
      throw new Error(`Required published renderer buffer is missing or invalid: ${name}. Run the owned renderer build/unpack command.`);
    }
    const bytes = await readFile(input);
    if (hash(bytes) !== expected.sha256) throw new Error(`Original renderer buffer hash mismatch: ${name}`);
    const id = `asset.source.render.buffer.${index++}`;
    assetIds[name] = id;
    // The public URL retains the manifest's gzip name; the server's physical-file
    // extension allowlist uses a .bin carrier. No content-encoding or bytes change.
    const path = name.endsWith(".gz") ? `assets/compiled/render/${name}.bin` : `assets/compiled/render/${name}`;
    await emit(id, `/assets/compiled/render/${name}`, path, bytes, "application/octet-stream");
  }
  const pack = JSON.parse(await readFile(resolve(root, "research/reference-pack/v1/manifest.json"), "utf8")) as {
    original_inputs: Array<{ id: string; settings: {
      camera_local_units: [number, number, number]; pitch_input: number; yaw_input: number;
      angle_units_per_turn: number; zoom: number; far_clip_units: number;
    } }>;
  };
  const fixtures: RendererDelivery["fixtures"] = {};
  for (const scene of manifest.scenes) {
    const input = pack.original_inputs.find((input) => input.id === `original.scenes.${scene.name}`);
    if (!input) throw new Error(`No approved original camera input for renderer fixture ${scene.name}.`);
    const settings = input.settings;
    fixtures[scene.name] = {
      sourceInputId: input.id,
      camera: {
        x: scene.base_x * 128 + settings.camera_local_units[0],
        height: settings.camera_local_units[1], y: scene.base_y * 128 + settings.camera_local_units[2],
        pitch: settings.pitch_input, yaw: settings.yaw_input, unitsPerTurn: 16384,
        zoom: settings.zoom, near: 50, far: settings.far_clip_units,
      },
      requiredAssets: [
        "asset.source.render.manifest", assetIds["palette.bin"]!,
        ...manifest.textures.map((id) => assetIds[`textures/${id}.bin`]!),
        ...manifest.npcs.map((npc) => assetIds[npc.pack]!),
        assetIds[scene.file_gz ?? scene.file]!, assetIds[scene.models_file_gz ?? scene.models_file]!,
      ],
    };
  }
  return {
    assets, files, bytes: total,
    renderer: { manifestSha256: RENDER_MANIFEST_SHA256, assetBaseUrl: "/assets/compiled/render/",
      assetIds, fixtures, coverage: "named_fixture_scenes_only" },
  };
}
