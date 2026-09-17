import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import type { RenderAssetManifest } from "../../../web/renderer/src/index.ts";
import { RENDER_MANIFEST_SHA256 } from "../../../web/app/render-identity.ts";
import { renderRuntimeFiles, verifyReproductionManifest } from "../render-data.ts";
import { validateRenderBlockPackage } from "../render-package.ts";
import { renderAssetContentType } from "../render-assets.ts";

async function published(): Promise<RenderAssetManifest> {
  const bytes = await readFile(new URL("../../../assets/compiled/render/manifest.json", import.meta.url));
  assert.equal(createHash("sha256").update(bytes).digest("hex"), RENDER_MANIFEST_SHA256);
  return JSON.parse(bytes.toString("utf8")) as RenderAssetManifest;
}

test("delivery follows the published runtime dependency graph and ships all 61 original blocks compressed", async () => {
  const manifest = await published();
  const runtime = renderRuntimeFiles(manifest);
  assert.equal(runtime.blocks.size, 61);
  assert.equal(runtime.scenes.size, 5);
  assert(runtime.blocks.has(12336));
  const blocks = [...runtime.blocks.values()].flat().filter((path) => path.endsWith(".gz"));
  assert.equal(blocks.length, 122);
  assert.equal(blocks.reduce((sum, path) => sum + manifest.files[path]!.size_bytes, 0), 57_357_774);
  assert(blocks.every((path) => path.endsWith(".gz")));
  assert(manifest.floor_definitions);
  assert(runtime.common.includes(manifest.floor_definitions.file));
  assert.equal(renderAssetContentType(manifest.floor_definitions.file), "application/octet-stream");
  assert.equal(manifest.gear_pose_fits?.sequences, 39);
  for (const sequence of [2305, 4847, 4850, 4853, 4855, 4857, 395, 400, 401, 428, 429, 440]) {
    assert(runtime.common.includes(`anim/seq-${sequence}.bin`));
  }
  for (const item of [1925, 5732, 9702]) assert(runtime.common.includes(`models/item-${item}-equip.bin`));
  assert(runtime.common.includes("minimap/mapscenes.bin"));
  assert(runtime.common.includes("minimap/mapicons.bin"));
  assert(runtime.common.includes(manifest.gear_pose_fits!.file));
  assert.equal(renderAssetContentType(manifest.gear_pose_fits!.file), "application/json");
  assert.equal(renderAssetContentType("minimap/mapicons.bin"), "application/octet-stream");
  assert.equal(renderAssetContentType("blocks/12336.bin.gz"), "application/octet-stream");
  assert.throws(() => renderAssetContentType("../private.rs"), /canonical/);
  assert.equal([...runtime.blocks.values()].flat().filter((path) => path.startsWith("minimap/blocks/")).length, 61);
  assert(runtime.files.every((path) => !path.startsWith("models/baked/") && path !== "tables.bin"));
  assert(runtime.files.every((path) => !path.startsWith("scenes/") || path.endsWith(".gz")));
  assert.equal(new Set(runtime.files).size, runtime.files.length);
  for (const file of runtime.common) assert(manifest.files[file]);
});

test("the current package binds all122 gzip twins and61 MICN sidecars to the entire current manifest", async () => {
  const manifest = await published();
  const index = JSON.parse(await readFile(new URL("../../../assets/compiled/render/blocks.index.json", import.meta.url), "utf8"));
  const result = validateRenderBlockPackage(index, manifest, RENDER_MANIFEST_SHA256);
  assert.equal(result.files.length, 183);
  assert.equal(result.pack.file, "clubscape-render-blocks-ffa5b7d7089c4a90.tar");
  assert.equal(result.pack.size_bytes, 58_030_080);
  assert.equal(result.pack.sha256, "880924edb755f3613a58d60b62d91c7507da968071998d2aaef91b77d7739b49");
  assert.throws(() => validateRenderBlockPackage({ ...index, manifest_sha256: "0".repeat(64) }, manifest, RENDER_MANIFEST_SHA256),
    /current manifest/);
  const missing = structuredClone(index);
  missing.files.pop();
  assert.throws(() => validateRenderBlockPackage(missing, manifest, RENDER_MANIFEST_SHA256), /exact source/);
  const oldSidecar = structuredClone(index);
  oldSidecar.files.find((entry: { file: string }) => entry.file.startsWith("minimap/blocks/")).sha256 = "0".repeat(64);
  assert.throws(() => validateRenderBlockPackage(oldSidecar, manifest, RENDER_MANIFEST_SHA256), /current published pin/);
  const raw = structuredClone(index);
  raw.files[0].decompressed_sha256 = "0".repeat(64);
  assert.throws(() => validateRenderBlockPackage(raw, manifest, RENDER_MANIFEST_SHA256), /raw\/gzip identity/);
});

test("block delivery requires the source floor definitions and their exact metadata/file pin", async () => {
  const original = await published();
  assert(original.floor_definitions);
  const missing = structuredClone(original);
  delete missing.floor_definitions;
  assert.throws(() => renderRuntimeFiles(missing), /require source floor definitions/);
  const unpinned = structuredClone(original);
  delete unpinned.files[original.floor_definitions.file];
  assert.throws(() => renderRuntimeFiles(unpinned), /floor definitions have no matching published pin/);
  const changed = structuredClone(original);
  changed.floor_definitions = { ...original.floor_definitions, sha256: "0".repeat(64) };
  assert.throws(() => renderRuntimeFiles(changed), /floor definitions have no matching published pin/);
  const omitted = structuredClone(original);
  delete omitted.files[original.floor_definitions.file];
  assert.throws(() => verifyReproductionManifest(original, omitted), /omitted a required published input/);
});

test("exporter inventory omission is allowed only for exact validation/raw twins, never changed metadata or runtime hashes", async () => {
  const original = await published();
  const reproduced = structuredClone(original);
  delete reproduced.files["tables.bin"];
  for (const path of Object.keys(reproduced.files)) {
    if (path.startsWith("models/baked/") || (path.startsWith("scenes/") && !path.endsWith(".gz"))) delete reproduced.files[path];
  }
  assert.equal(verifyReproductionManifest(original, reproduced).length, 65);
  const changed = structuredClone(reproduced);
  changed.files["palette.bin"]!.sha256 = "0".repeat(64);
  assert.throws(() => verifyReproductionManifest(original, changed), /changed a published input/);
  delete reproduced.files["blocks/12336.bin.gz"];
  assert.throws(() => verifyReproductionManifest(original, reproduced), /omitted a required/);
  const other = structuredClone(original);
  other.brightness = 0.7;
  assert.throws(() => verifyReproductionManifest(original, other), /changed source renderer metadata/);
});
