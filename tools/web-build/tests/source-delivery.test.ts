import assert from "node:assert/strict";
import { test } from "node:test";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { readArtifact } from "../artifact.ts";
import { createHash } from "node:crypto";

test("current canonical artifact uses the native-ready source hash, not an obsolete failure pin", async () => {
  const root = new URL("../../../", import.meta.url);
  const manifest = JSON.parse(await readFile(new URL("content/m1/manifest.json", root), "utf8")) as {
    compiled_artifact: { path: string; uncompressed_sha256: string };
  };
  const bytes = await readArtifact(fileURLToPath(new URL(manifest.compiled_artifact.path, root)));
  assert.equal(createHash("sha256").update(bytes).digest("hex"), manifest.compiled_artifact.uncompressed_sha256);
});
