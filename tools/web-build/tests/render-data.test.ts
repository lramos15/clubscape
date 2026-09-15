import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import type { RenderAssetManifest } from "../../../web/renderer/src/index.ts";
import { RENDER_MANIFEST_SHA256 } from "../../../web/app/render-identity.ts";
import { renderRuntimeFiles, verifyReproductionManifest } from "../render-data.ts";

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
  assert.equal(blocks.reduce((sum, path) => sum + manifest.files[path]!.size_bytes, 0), 55_720_421);
  assert(blocks.every((path) => path.endsWith(".gz")));
  assert(runtime.common.includes("minimap/mapscenes.bin"));
  assert.equal([...runtime.blocks.values()].flat().filter((path) => path.startsWith("minimap/blocks/")).length, 61);
  assert(runtime.files.every((path) => !path.startsWith("models/baked/") && path !== "tables.bin"));
  assert(runtime.files.every((path) => !path.startsWith("scenes/") || path.endsWith(".gz")));
  assert.equal(new Set(runtime.files).size, runtime.files.length);
  for (const file of runtime.common) assert(manifest.files[file]);
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
