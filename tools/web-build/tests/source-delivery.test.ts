import assert from "node:assert/strict";
import { test } from "node:test";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { readArtifact } from "../artifact.ts";
import { createHash } from "node:crypto";

test("current canonical artifact uses the native-ready source hash, not an obsolete failure pin", async () => {
  const root = new URL("../../../", import.meta.url);
  const manifest = JSON.parse(await readFile(new URL("content/m1/manifest.json", root), "utf8")) as {
    compiled_artifact: { path: string; sha256: string; bytes: number; uncompressed_sha256: string; uncompressed_bytes: number };
  };
  const artifact = manifest.compiled_artifact;
  const path = new URL(artifact.path, root);
  const compressed = await readFile(path);
  assert.equal(compressed.length, artifact.bytes);
  assert.equal(createHash("sha256").update(compressed).digest("hex"), artifact.sha256);
  const bytes = await readArtifact(fileURLToPath(path));
  assert.equal(bytes.length, artifact.uncompressed_bytes);
  assert.equal(createHash("sha256").update(bytes).digest("hex"), artifact.uncompressed_sha256);
});
